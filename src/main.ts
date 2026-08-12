import "./styles.css";

import { invoke } from "@tauri-apps/api/core";

type ProfileView = {
  id: string;
  label: string;
  path: string;
  is_default: boolean;
  shares_account: boolean;
};

const profilesElement = document.querySelector<HTMLUListElement>("#profiles");
const countElement = document.querySelector<HTMLSpanElement>("#profile-count");
const errorElement = document.querySelector<HTMLDivElement>("#error");
const addForm = document.querySelector<HTMLFormElement>("#add-profile-form");
const labelInput = document.querySelector<HTMLInputElement>("#new-label");

if (!profilesElement || !countElement || !errorElement || !addForm || !labelInput) {
  throw new Error("Claude Profiles management window is missing required elements");
}

const profilesList = profilesElement;
const profileCount = countElement;
const errorBox = errorElement;
const profileForm = addForm;
const profileLabelInput = labelInput;

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function showError(error: unknown): void {
  errorBox.textContent = errorMessage(error);
  errorBox.hidden = false;
}

function clearError(): void {
  errorBox.textContent = "";
  errorBox.hidden = true;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = -1;
  do {
    value /= 1024;
    unit += 1;
  } while (value >= 1024 && unit < units.length - 1);
  return `${value.toFixed(value >= 10 ? 0 : 1)} ${units[unit]}`;
}

function makeTextElement(tag: "h3" | "p" | "span", className: string, text: string): HTMLElement {
  const element = document.createElement(tag);
  element.className = className;
  element.textContent = text;
  return element;
}

function render(profiles: ProfileView[]): void {
  profilesList.replaceChildren();
  profileCount.textContent = String(profiles.length);

  for (const profile of profiles) {
    const item = document.createElement("li");
    item.className = "profile-card";

    const index = makeTextElement("span", "profile-index", String(profiles.indexOf(profile) + 1).padStart(2, "0"));
    const content = document.createElement("div");
    content.className = "profile-content";
    const title = document.createElement("div");
    title.className = "profile-title";
    title.append(makeTextElement("h3", "profile-label", profile.label));
    if (profile.is_default) {
      title.append(makeTextElement("span", "status-badge status-default", "Default"));
    }
    if (profile.shares_account) {
      title.append(makeTextElement("span", "status-badge status-warning", "same account"));
    }
    content.append(title);
    content.append(makeTextElement("p", "profile-path", profile.path));

    const actions = document.createElement("div");
    actions.className = "profile-actions";

    const renameButton = document.createElement("button");
    renameButton.className = "button button-quiet";
    renameButton.type = "button";
    renameButton.textContent = "Rename";
    renameButton.addEventListener("click", () => startRename(profile, content));
    actions.append(renameButton);

    // The Default profile is the existing Claude Desktop installation, so its
    // directory is never ours to delete. Its label is still just a label.
    if (!profile.is_default) {
      const deleteButton = document.createElement("button");
      deleteButton.className = "button button-danger";
      deleteButton.type = "button";
      deleteButton.textContent = "Delete";
      deleteButton.addEventListener("click", () => startDelete(profile, content));
      actions.append(deleteButton);
    }

    item.append(index, content, actions);
    profilesList.append(item);
  }
}

/// Rename and delete both used to call `window.prompt` / `window.confirm`.
/// Tauri's webview does not implement either one, so both actions silently did
/// nothing. Everything below is drawn in the page instead.
function startRename(profile: ProfileView, content: HTMLElement): void {
  if (content.querySelector(".inline-panel")) return;

  const panel = document.createElement("form");
  panel.className = "inline-panel";

  const input = document.createElement("input");
  input.type = "text";
  input.maxLength = 80;
  input.value = profile.label;
  input.setAttribute("aria-label", `New label for ${profile.label}`);

  const save = document.createElement("button");
  save.type = "submit";
  save.className = "button button-primary";
  save.textContent = "Save";

  const cancel = document.createElement("button");
  cancel.type = "button";
  cancel.className = "button button-quiet";
  cancel.textContent = "Cancel";
  cancel.addEventListener("click", () => panel.remove());

  panel.append(input, save, cancel);
  panel.addEventListener("submit", async (event) => {
    event.preventDefault();
    const label = input.value.trim();
    if (!label || label === profile.label) {
      panel.remove();
      return;
    }
    try {
      await invoke("rename_profile", { id: profile.id, label });
      clearError();
      await loadProfiles();
    } catch (error) {
      showError(error);
    }
  });

  content.append(panel);
  input.focus();
  input.select();
}

async function startDelete(profile: ProfileView, content: HTMLElement): Promise<void> {
  if (content.querySelector(".inline-panel")) return;

  let size: number;
  try {
    size = await invoke<number>("profile_size_bytes", { id: profile.id });
  } catch (error) {
    showError(error);
    return;
  }

  const panel = document.createElement("div");
  panel.className = "inline-panel inline-panel-danger";
  panel.append(
    makeTextElement(
      "p",
      "helper",
      `Delete “${profile.label}” and all ${formatBytes(size)} in ${profile.path}? This cannot be undone.`,
    ),
  );

  const confirm = document.createElement("button");
  confirm.type = "button";
  confirm.className = "button button-danger";
  confirm.textContent = "Delete permanently";
  confirm.addEventListener("click", async () => {
    try {
      await invoke("delete_profile", { id: profile.id });
      clearError();
      await loadProfiles();
    } catch (error) {
      showError(error);
    }
  });

  const cancel = document.createElement("button");
  cancel.type = "button";
  cancel.className = "button button-quiet";
  cancel.textContent = "Keep it";
  cancel.addEventListener("click", () => panel.remove());

  panel.append(confirm, cancel);
  content.append(panel);
  confirm.focus();
}

async function loadProfiles(): Promise<void> {
  try {
    const profiles = await invoke<ProfileView[]>("list_profiles");
    render(profiles);
    clearError();
  } catch (error) {
    showError(error);
  }
}

async function addProfile(event: SubmitEvent): Promise<void> {
  event.preventDefault();
  const label = profileLabelInput.value.trim();
  if (!label) {
    showError("Enter a label for this profile.");
    profileLabelInput.focus();
    return;
  }

  try {
    await invoke("add_profile", { label });
    profileLabelInput.value = "";
    await loadProfiles();
  } catch (error) {
    showError(error);
  }
}

// A desktop app has no business offering "Reload" or "Inspect Element" on
// right-click. Keep the caret menu inside text fields, where it is useful.
document.addEventListener("contextmenu", (event) => {
  const target = event.target as HTMLElement | null;
  if (target?.closest("input, textarea")) return;
  event.preventDefault();
});

profileForm.addEventListener("submit", addProfile);
void loadProfiles();

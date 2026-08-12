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
    if (!profile.is_default) {
      const renameButton = document.createElement("button");
      renameButton.className = "button button-quiet";
      renameButton.type = "button";
      renameButton.textContent = "Rename";
      renameButton.addEventListener("click", () => renameProfile(profile));
      actions.append(renameButton);

      const deleteButton = document.createElement("button");
      deleteButton.className = "button button-danger";
      deleteButton.type = "button";
      deleteButton.textContent = "Delete";
      deleteButton.addEventListener("click", () => deleteProfile(profile));
      actions.append(deleteButton);
    }

    item.append(index, content, actions);
    profilesList.append(item);
  }
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

async function renameProfile(profile: ProfileView): Promise<void> {
  const label = window.prompt("Profile label", profile.label)?.trim();
  if (!label || label === profile.label) return;

  try {
    await invoke("rename_profile", { id: profile.id, label });
    await loadProfiles();
  } catch (error) {
    showError(error);
  }
}

async function deleteProfile(profile: ProfileView): Promise<void> {
  try {
    const size = await invoke<number>("profile_size_bytes", { id: profile.id });
    const confirmed = window.confirm(
      `Delete the profile “${profile.label}” and all ${formatBytes(size)} in:\n${profile.path}?\n\nThis cannot be undone.`,
    );
    if (!confirmed) return;

    await invoke("delete_profile", { id: profile.id });
    await loadProfiles();
  } catch (error) {
    showError(error);
  }
}

profileForm.addEventListener("submit", addProfile);
void loadProfiles();

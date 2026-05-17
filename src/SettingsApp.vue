<script setup>
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { computed, onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { setLocale } from "./i18n";

const { t } = useI18n();

const activeTab = ref("general");
const savedLocale = ref("en");
const locale = ref("en");
const launchAtLogin = ref(false);
const saveMode = ref("duration");
const retentionDuration = ref(30);
const retentionCount = ref(500);
const cleanupTime = ref("00:00");
const shortcutShow = ref("CmdOrCtrl+Shift+V");
const shortcutHide = ref("Escape");
const recordingField = ref(null);

const tabs = computed(() => [
  { id: "general", label: t("settings.general") },
  { id: "shortcuts", label: t("settings.shortcuts") },
  { id: "update", label: t("settings.update") },
  { id: "about", label: t("settings.about") },
]);

onMounted(async () => {
  await loadSettings();
  document.addEventListener("keydown", handleKeydown);
});

onUnmounted(() => {
  document.removeEventListener("keydown", handleKeydown);
});

async function loadSettings() {
  try {
    const settings = await invoke("get_settings");
    if (settings) {
      savedLocale.value = settings.locale || "en";
      locale.value = savedLocale.value;
      shortcutShow.value = settings.shortcut_show || "CmdOrCtrl+Shift+V";
      shortcutHide.value = settings.shortcut_hide || "Escape";
      launchAtLogin.value = settings.launch_at_login || false;
      saveMode.value = settings.save_mode || "duration";
      retentionDuration.value = settings.retention_duration || 30;
      retentionCount.value = settings.retention_count || 500;
      cleanupTime.value = settings.cleanup_time || "00:00";
      setLocale(savedLocale.value);
      updateWindowTitle();
    }
  } catch (e) {
    console.error("Failed to load settings:", e);
  }
}

function updateWindowTitle() {
  const win = getCurrentWindow();
  const title = savedLocale.value === "zh" ? "首选项" : "Preferences";
  win.setTitle(title);
}

function startRecording(field) {
  recordingField.value = field;
}

function stopRecording() {
  recordingField.value = null;
}

function handleKeydown(e) {
  if (!recordingField.value) return;

  e.preventDefault();
  e.stopPropagation();

  if (e.key === "Escape") {
    if (e.ctrlKey || e.metaKey || e.altKey || e.shiftKey) {
      const parts = buildShortcutParts(e);
      recordingField.value === "show"
        ? (shortcutShow.value = parts)
        : (shortcutHide.value = parts);
    } else {
      if (recordingField.value === "hide") {
        shortcutHide.value = "Escape";
      }
    }
    stopRecording();
    return;
  }

  if (e.key === "Tab" || e.key === "CapsLock") return;

  const parts = buildShortcutParts(e);
  if (parts) {
    recordingField.value === "show"
      ? (shortcutShow.value = parts)
      : (shortcutHide.value = parts);
    stopRecording();
  }
}

function buildShortcutParts(e) {
  const mods = [];
  if (e.metaKey) mods.push("Cmd");
  else if (e.ctrlKey) mods.push("Ctrl");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");

  const keyName = mapKey(e.key, e.code);
  if (!keyName) return null;

  if (mods.length === 0 && !isModifierKey(e.key)) {
    return null;
  }

  return [...mods, keyName].join("+");
}

function isModifierKey(key) {
  return ["Control", "Alt", "Shift", "Meta"].includes(key);
}

function mapKey(key, code) {
  if (key.length === 1) return key.toUpperCase();
  if (key.startsWith("F") && key.length <= 3 && !isNaN(key.slice(1))) return key;
  if (key === " ") return "Space";
  if (key === "Enter") return "Enter";
  if (key === "Backspace") return "Backspace";
  if (key === "Delete") return "Delete";
  if (key === "ArrowUp") return "Up";
  if (key === "ArrowDown") return "Down";
  if (key === "ArrowLeft") return "Left";
  if (key === "ArrowRight") return "Right";
  if (key === "Home") return "Home";
  if (key === "End") return "End";
  if (key === "PageUp") return "PageUp";
  if (key === "PageDown") return "PageDown";
  if (key === "Escape") return "Escape";
  if (code) {
    if (code.startsWith("BracketLeft")) return "[";
    if (code.startsWith("BracketRight")) return "]";
    if (code.startsWith("Semicolon")) return ";";
    if (code.startsWith("Quote")) return "'";
    if (code.startsWith("Comma")) return ",";
    if (code.startsWith("Period")) return ".";
    if (code.startsWith("Slash")) return "/";
    if (code.startsWith("Backslash")) return "\\";
    if (code.startsWith("Minus")) return "-";
    if (code.startsWith("Equal")) return "=";
    if (code.startsWith("Backquote")) return "`";
  }
  return null;
}

async function saveSettings() {
  try {
    savedLocale.value = locale.value;
    await invoke("save_settings", {
      settings: {
        save_mode: saveMode.value,
        retention_duration: retentionDuration.value,
        retention_count: retentionCount.value,
        cleanup_time: cleanupTime.value,
        shortcut_show: shortcutShow.value,
        shortcut_hide: shortcutHide.value,
        locale: locale.value,
        launch_at_login: launchAtLogin.value,
      },
    });
    setLocale(locale.value);
    updateWindowTitle();
    await invoke("close_settings_window");
  } catch (e) {
    console.error("Failed to save settings:", e);
    alert("Failed to save settings: " + e);
  }
}

function cancelSettings() {
  locale.value = savedLocale.value;
  invoke("close_settings_window");
}
</script>

<template>
  <main class="settings-container">
    <div class="settings-tabs">
      <button
        v-for="tab in tabs"
        :key="tab.id"
        :class="['tab-btn', { active: activeTab === tab.id }]"
        @click="activeTab = tab.id"
      >
        {{ tab.label }}
      </button>
    </div>

    <div class="settings-body">
      <div v-if="activeTab === 'general'" class="tab-content">
        <div class="setting-row">
          <label class="setting-label">{{ t("settings.language") }}</label>
          <select v-model="locale" class="setting-control">
            <option value="en">English</option>
            <option value="zh">简体中文</option>
          </select>
        </div>

        <div class="setting-row">
          <label class="setting-label">
            <input type="checkbox" v-model="launchAtLogin" class="setting-checkbox" />
            {{ t("settings.launchAtLogin") }}
          </label>
        </div>

        <div class="setting-section-title">{{ t("settings.dataRetention") }}</div>

        <div class="setting-row">
          <label class="setting-label">{{ t("settings.saveMode") }}</label>
          <select v-model="saveMode" class="setting-control">
            <option value="duration">{{ t("settings.byDuration") }}</option>
            <option value="count">{{ t("settings.byCount") }}</option>
          </select>
        </div>

        <div v-if="saveMode === 'duration'" class="setting-row">
          <label class="setting-label">{{ t("settings.retentionDuration") }}</label>
          <div class="setting-input-group">
            <input type="number" v-model.number="retentionDuration" min="1" max="365" class="setting-input setting-input-sm" />
            <span class="setting-unit">{{ t("settings.days") }}</span>
          </div>
        </div>

        <div v-if="saveMode === 'count'" class="setting-row">
          <label class="setting-label">{{ t("settings.retentionCount") }}</label>
          <div class="setting-input-group">
            <input type="number" v-model.number="retentionCount" min="10" max="10000" class="setting-input setting-input-sm" />
            <span class="setting-unit">{{ t("settings.items") }}</span>
          </div>
        </div>

        <div class="setting-row">
          <label class="setting-label">{{ t("settings.cleanupTime") }}</label>
          <input type="time" v-model="cleanupTime" class="setting-control" />
        </div>
      </div>

      <div v-if="activeTab === 'shortcuts'" class="tab-content">
        <div class="setting-row">
          <label class="setting-label">{{ t("settings.showPanel") }}</label>
          <button
            :class="['shortcut-badge', 'shortcut-btn', { recording: recordingField === 'show' }]"
            @click="startRecording('show')"
          >
            <template v-if="recordingField === 'show'">{{ t("settings.recording") }}</template>
            <template v-else>{{ shortcutShow }}</template>
          </button>
        </div>
        <div class="setting-row">
          <label class="setting-label">{{ t("settings.hidePanel") }}</label>
          <button
            :class="['shortcut-badge', 'shortcut-btn', { recording: recordingField === 'hide' }]"
            @click="startRecording('hide')"
          >
            <template v-if="recordingField === 'hide'">{{ t("settings.recording") }}</template>
            <template v-else>{{ shortcutHide }}</template>
          </button>
        </div>
        <p class="shortcut-hint">{{ t("settings.shortcutHint") }}</p>
      </div>

      <div v-if="activeTab === 'update'" class="tab-content">
        <div class="placeholder-content">
          <p>{{ t("settings.updatePlaceholder") }}</p>
        </div>
      </div>

      <div v-if="activeTab === 'about'" class="tab-content">
        <div class="placeholder-content">
          <p>{{ t("settings.aboutPlaceholder") }}</p>
        </div>
      </div>
    </div>

    <div class="settings-footer">
      <button class="btn-cancel" @click="cancelSettings">{{ t("settings.cancel") }}</button>
      <button class="btn-save" @click="saveSettings">{{ t("settings.save") }}</button>
    </div>
  </main>
</template>

<style>
:root {
  font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", sans-serif;
  font-size: 13px;
  line-height: 1.5;
  color: #1a1a1a;
  background: #f5f5f7;
  margin: 0;
  padding: 0;
}

* {
  box-sizing: border-box;
}

body {
  margin: 0;
  padding: 0;
}

.settings-container {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: #f5f5f7;
}

.settings-tabs {
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 12px 16px 0;
  background: #f5f5f7;
  position: relative;
  z-index: 2;
}

.tab-btn {
  padding: 6px 16px;
  border: 1px solid #d1d1d6;
  border-bottom: none;
  border-radius: 8px 8px 0 0;
  background: #e8e8ed;
  color: #666;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s ease;
  position: relative;
}

.tab-btn:hover {
  background: #e0e0e5;
}

.tab-btn.active {
  background: #ffffff;
  color: #1a1a1a;
  border-bottom-color: #ffffff;
  margin-bottom: -1px;
  padding-bottom: 7px;
}

.settings-body {
  flex: 1;
  background: #ffffff;
  margin: 0 16px;
  padding: 20px 24px;
  border: 1px solid #d1d1d6;
  border-radius: 0 0 8px 8px;
  overflow-y: auto;
  position: relative;
  z-index: 1;
}

.tab-content {
  padding: 4px 0;
}

.setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 36px;
  padding: 6px 0;
}

.setting-label {
  font-size: 13px;
  color: #1a1a1a;
  display: flex;
  align-items: center;
  gap: 8px;
}

.setting-control {
  padding: 6px 12px;
  border: 1px solid #d1d1d6;
  border-radius: 6px;
  font-size: 13px;
  color: #1a1a1a;
  background: #ffffff;
  min-width: 160px;
  height: 32px;
}

.setting-control:focus {
  outline: none;
  border-color: #007aff;
  box-shadow: 0 0 0 3px rgba(0, 122, 255, 0.15);
}

.setting-checkbox {
  width: 16px;
  height: 16px;
  accent-color: #007aff;
  cursor: pointer;
}

.shortcut-badge {
  display: inline-flex;
  align-items: center;
  padding: 4px 12px;
  background: #f5f5f7;
  border: 1px solid #d1d1d6;
  border-radius: 6px;
  font-size: 13px;
  font-family: -apple-system, BlinkMacSystemFont, "SF Mono", monospace;
  color: #666;
  min-width: 120px;
  justify-content: center;
  height: 32px;
}

.shortcut-btn {
  cursor: pointer;
  transition: all 0.15s ease;
  font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", sans-serif;
}

.shortcut-btn:hover {
  border-color: #007aff;
  color: #007aff;
  background: #f0f5ff;
}

.shortcut-btn.recording {
  border-color: #007aff;
  background: #007aff;
  color: #ffffff;
  animation: pulse 1.5s ease-in-out infinite;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.7; }
}

.shortcut-hint {
  font-size: 11px;
  color: #999;
  margin: 8px 0 0;
}

.setting-section-title {
  font-size: 12px;
  font-weight: 600;
  color: #999;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  margin-top: 16px;
  margin-bottom: 4px;
  padding-bottom: 4px;
  border-bottom: 1px solid #f0f0f0;
}

.setting-input-group {
  display: flex;
  align-items: center;
  gap: 6px;
}

.setting-input {
  padding: 6px 12px;
  border: 1px solid #d1d1d6;
  border-radius: 6px;
  font-size: 13px;
  color: #1a1a1a;
  background: #ffffff;
  height: 32px;
}

.setting-input:focus {
  outline: none;
  border-color: #007aff;
  box-shadow: 0 0 0 3px rgba(0, 122, 255, 0.15);
}

.setting-input-sm {
  width: 141px;
  text-align: right;
}

.setting-unit {
  font-size: 13px;
  color: #666;
}

.placeholder-content {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 200px;
  color: #999;
  font-size: 14px;
}

.settings-footer {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 10px;
  padding: 12px 16px;
  background: #f5f5f7;
}

.btn-cancel,
.btn-save {
  padding: 6px 16px;
  border-radius: 6px;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  height: 32px;
  transition: all 0.15s ease;
}

.btn-cancel {
  background: #ffffff;
  border: 1px solid #d1d1d6;
  color: #1a1a1a;
}

.btn-cancel:hover {
  background: #f5f5f7;
}

.btn-save {
  background: #007aff;
  border: none;
  color: #ffffff;
}

.btn-save:hover {
  background: #0066d6;
}
</style>

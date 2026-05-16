<script setup>
import { ref, watch } from "vue";

const props = defineProps({
  show: Boolean,
  saveMode: String,
  retentionDuration: Number,
  retentionCount: Number,
  cleanupTime: String,
  shortcutShow: String,
  shortcutPin: String,
});

const emit = defineEmits(["close", "save"]);

const localSaveMode = ref(props.saveMode);
const localRetentionDuration = ref(props.retentionDuration);
const localRetentionCount = ref(props.retentionCount);
const localCleanupTime = ref(props.cleanupTime);
const localShortcutShow = ref(props.shortcutShow);
const localShortcutPin = ref(props.shortcutPin);

watch(() => props.show, (newVal) => {
  if (newVal) {
    localSaveMode.value = props.saveMode;
    localRetentionDuration.value = props.retentionDuration;
    localRetentionCount.value = props.retentionCount;
    localCleanupTime.value = props.cleanupTime;
    localShortcutShow.value = props.shortcutShow;
    localShortcutPin.value = props.shortcutPin;
  }
});

function close() {
  emit("close");
}

function save() {
  emit("save", {
    save_mode: localSaveMode.value,
    retention_duration: localRetentionDuration.value,
    retention_count: localRetentionCount.value,
    cleanup_time: localCleanupTime.value,
    shortcut_show: localShortcutShow.value,
    shortcut_pin: localShortcutPin.value,
  });
}
</script>

<template>
  <div v-if="show" class="settings-overlay" @click.self="close">
    <div class="settings-modal">
      <div class="settings-header">
        <h2>Settings</h2>
        <button class="close-btn" @click="close">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="18" y1="6" x2="6" y2="18"></line>
            <line x1="6" y1="6" x2="18" y2="18"></line>
          </svg>
        </button>
      </div>

      <div class="settings-content">
        <div class="settings-section">
          <h3>Data Retention</h3>
          <div class="setting-row">
            <label>Save Mode</label>
            <div class="radio-group">
              <label class="radio-label">
                <input type="radio" v-model="localSaveMode" value="duration" />
                <span>By Duration</span>
              </label>
              <label class="radio-label">
                <input type="radio" v-model="localSaveMode" value="count" />
                <span>By Count</span>
              </label>
            </div>
          </div>

          <div class="setting-row" v-if="localSaveMode === 'duration'">
            <label>Retention Period</label>
            <select v-model="localRetentionDuration">
              <option :value="7">1 Week</option>
              <option :value="14">2 Weeks</option>
              <option :value="30">1 Month</option>
              <option :value="60">2 Months</option>
              <option :value="90">3 Months</option>
            </select>
          </div>

          <div class="setting-row" v-if="localSaveMode === 'count'">
            <label>Max Items</label>
            <select v-model="localRetentionCount">
              <option :value="100">100</option>
              <option :value="200">200</option>
              <option :value="500">500</option>
              <option :value="1000">1000</option>
              <option :value="2000">2000</option>
            </select>
          </div>

          <div class="setting-row">
            <label>Daily Cleanup Time</label>
            <input type="time" v-model="localCleanupTime" />
          </div>
        </div>

        <div class="settings-section">
          <h3>Shortcuts</h3>
          <div class="setting-row">
            <label>Show Panel</label>
            <input type="text" v-model="localShortcutShow" readonly class="shortcut-input" />
          </div>
          <div class="setting-row">
            <label>Pin Item</label>
            <input type="text" v-model="localShortcutPin" readonly class="shortcut-input" />
          </div>
        </div>
      </div>

      <div class="settings-footer">
        <button class="btn-cancel" @click="close">Cancel</button>
        <button class="btn-save" @click="save">Save</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.4);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
  backdrop-filter: blur(4px);
}

.settings-modal {
  background: #ffffff;
  border-radius: 16px;
  width: 420px;
  max-width: 90vw;
  max-height: 80vh;
  overflow: hidden;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.2);
  display: flex;
  flex-direction: column;
}

.settings-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 20px 24px;
  border-bottom: 1px solid #eee;
}

.settings-header h2 {
  margin: 0;
  font-size: 18px;
  font-weight: 600;
  color: #1a1a1a;
}

.close-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  background: transparent;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  color: #666;
  transition: all 0.2s ease;
}

.close-btn:hover {
  background: rgba(0, 0, 0, 0.05);
  color: #333;
}

.close-btn svg {
  width: 18px;
  height: 18px;
}

.settings-content {
  flex: 1;
  overflow-y: auto;
  padding: 24px;
}

.settings-section {
  margin-bottom: 28px;
}

.settings-section:last-child {
  margin-bottom: 0;
}

.settings-section h3 {
  margin: 0 0 16px 0;
  font-size: 14px;
  font-weight: 600;
  color: #666;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 16px;
}

.setting-row:last-child {
  margin-bottom: 0;
}

.setting-row label {
  font-size: 14px;
  color: #333;
}

.setting-row select,
.setting-row input[type="time"] {
  padding: 8px 12px;
  border: 1px solid #ddd;
  border-radius: 8px;
  font-size: 14px;
  color: #333;
  background: #fff;
  min-width: 140px;
}

.setting-row select:focus,
.setting-row input[type="time"]:focus {
  outline: none;
  border-color: #4a90d9;
}

.radio-group {
  display: flex;
  gap: 16px;
}

.radio-label {
  display: flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
  font-size: 14px;
  color: #333;
}

.radio-label input[type="radio"] {
  width: 16px;
  height: 16px;
  accent-color: #4a90d9;
}

.shortcut-input {
  padding: 8px 12px;
  border: 1px solid #ddd;
  border-radius: 8px;
  font-size: 14px;
  color: #666;
  background: #f5f5f5;
  min-width: 140px;
  text-align: center;
}

.settings-footer {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 12px;
  padding: 16px 24px;
  border-top: 1px solid #eee;
}

.btn-cancel,
.btn-save {
  padding: 10px 20px;
  border-radius: 8px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s ease;
}

.btn-cancel {
  background: transparent;
  border: 1px solid #ddd;
  color: #666;
}

.btn-cancel:hover {
  background: #f5f5f5;
  border-color: #ccc;
}

.btn-save {
  background: #4a90d9;
  border: none;
  color: #fff;
}

.btn-save:hover {
  background: #3a7bc8;
}
</style>
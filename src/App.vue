<script setup>
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { nextTick, onMounted, onUnmounted, ref } from "vue";

const clipboardItems = ref([]);
const maxItems = 30;
const imageUrls = ref({});
const searchQuery = ref("");
const isSearchFocused = ref(false);
const searchInput = ref(null);
const clipboardList = ref(null);
const isDragging = ref(false);
const dragStartX = ref(0);
const scrollStartX = ref(0);
const tags = ref([
  { id: 1, name: "Clipboard", isDefault: true },
  { id: 2, name: "Work", isDefault: false, color: "#4a90d9" },
  { id: 3, name: "Personal", isDefault: false, color: "#50c878" },
]);
const activeTag = ref(1);
const showAddTag = ref(false);
const newTagName = ref("");
const newTagInput = ref(null);
const selectedItemId = ref(null);

const typeColors = {
  text: { header: "#4a90d9", icon: "#ffffff" },
  image: { header: "#50c878", icon: "#ffffff" },
};

onMounted(async () => {
  await listen("clipboard-change", (event) => {
    const payload = event.payload;
    const item = {
      id: Date.now() + Math.random(),
      content_type: payload.content_type,
      content: payload.content,
      timestamp: payload.timestamp,
      isNew: true,
    };
    clipboardItems.value.unshift(item);

    if (payload.content_type === "image" && payload.content?.Image) {
      const { width, height, rgba } = payload.content.Image;
      getImageBlob(rgba, width, height, (blob) => {
        if (blob) {
          const url = URL.createObjectURL(blob);
          imageUrls.value[item.id] = url;
        }
      });
    }

    if (clipboardItems.value.length > maxItems) {
      const removed = clipboardItems.value.pop();
      if (removed && imageUrls.value[removed.id]) {
        URL.revokeObjectURL(imageUrls.value[removed.id]);
        delete imageUrls.value[removed.id];
      }
    }
    setTimeout(() => {
      const newItem = clipboardItems.value.find((i) => i.id === item.id);
      if (newItem) {
        newItem.isNew = false;
      }
    }, 500);
  });
});

function onWheel(e) {
  if (!clipboardList.value) return;
  e.preventDefault();
  clipboardList.value.scrollLeft += e.deltaY;
}

function onDragStart(e) {
  if (!clipboardList.value) return;
  isDragging.value = true;
  dragStartX.value = e.clientX;
  scrollStartX.value = clipboardList.value.scrollLeft;
}

function onDragMove(e) {
  if (!isDragging.value || !clipboardList.value) return;
  const dx = e.clientX - dragStartX.value;
  clipboardList.value.scrollLeft = scrollStartX.value - dx;
}

function onDragEnd() {
  isDragging.value = false;
}

onUnmounted(() => {
  isDragging.value = false;
});

function focusSearch() {
  isSearchFocused.value = true;
  nextTick(() => {
    if (searchInput.value) {
      searchInput.value.focus();
    }
  });
}

function blurSearch() {
  if (!searchQuery.value) {
    isSearchFocused.value = false;
  }
}

function startAddTag() {
  showAddTag.value = true;
  nextTick(() => {
    if (newTagInput.value) {
      newTagInput.value.focus();
    }
  });
}

function finishAddTag() {
  if (newTagName.value.trim()) {
    const colors = ["#4a90d9", "#50c878", "#e74c3c", "#9b59b6", "#f39c12", "#1abc9c"];
    const color = colors[tags.value.length % colors.length];
    tags.value.push({
      id: Date.now(),
      name: newTagName.value.trim(),
      isDefault: false,
      color,
    });
  }
  newTagName.value = "";
  showAddTag.value = false;
}

function cancelAddTag() {
  newTagName.value = "";
  showAddTag.value = false;
}

function selectTag(tagId) {
  activeTag.value = tagId;
}

function selectItem(itemId) {
  selectedItemId.value = selectedItemId.value === itemId ? null : itemId;
}

async function pasteItem(item) {
  try {
    await invoke("paste_item", { item });
  } catch (e) {
    console.error("Failed to paste item:", e);
  }
}

function getTextContent(item) {
  if (!item.content) return "";
  if (typeof item.content === "string") return item.content;
  if (item.content.Text) return item.content.Text;
  return "";
}

function getImageBlob(rgba, width, height, callback) {
  try {
    if (!width || !height || !rgba || rgba.length === 0) {
      callback(null);
      return;
    }
    const canvas = document.createElement("canvas");
    canvas.width = Number(width);
    canvas.height = Number(height);
    const ctx = canvas.getContext("2d");
    const imageData = ctx.createImageData(Number(width), Number(height));
    for (let i = 0; i < rgba.length; i++) {
      imageData.data[i] = rgba[i];
    }
    ctx.putImageData(imageData, 0, 0);
    canvas.toBlob((blob) => {
      callback(blob);
    }, "image/png");
  } catch (e) {
    console.error("Error creating blob:", e);
    callback(null);
  }
}

function formatTime(timestamp) {
  const date = new Date(timestamp);
  const now = new Date();
  const diff = now - date;

  if (diff < 60000) return "Just now";
  if (diff < 3600000) return `${Math.floor(diff / 60000)}m ago`;
  if (diff < 86400000) return `${Math.floor(diff / 3600000)}h ago`;

  return date.toLocaleDateString();
}

function getCharCount(text) {
  if (!text) return 0;
  return text.length;
}

function getImageDimensions(item) {
  if (!item.content?.Image) return "";
  return `${item.content.Image.width} × ${item.content.Image.height}`;
}

function truncateText(text, maxChars = 200) {
  if (!text) return "";
  if (text.length <= maxChars) return text;
  return text.slice(0, maxChars) + "...";
}

function filterItems() {
  let items = clipboardItems.value;
  if (searchQuery.value) {
    items = items.filter((item) => {
      if (item.content_type === "text") {
        return getTextContent(item).toLowerCase().includes(searchQuery.value.toLowerCase());
      }
      return false;
    });
  }
  return items;
}
</script>

<template>
  <main class="container">
    <div class="top-bar">
      <div class="search-input-wrapper" :class="{ active: isSearchFocused }" @click="focusSearch">
        <svg class="search-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="11" cy="11" r="8"></circle>
          <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
        </svg>
        <input
          ref="searchInput"
          v-model="searchQuery"
          type="text"
          class="search-input"
          placeholder="Search..."
          @blur="blurSearch"
          @keydown.esc="blurSearch"
        />
      </div>
      <div class="tags-container">
        <div
          v-for="tag in tags"
          :key="tag.id"
          class="tag"
          :class="{ active: activeTag === tag.id }"
          @click="selectTag(tag.id)"
        >
          <span v-if="!tag.isDefault" class="tag-dot" :style="{ backgroundColor: tag.color }"></span>
          <span class="tag-name">{{ tag.name }}</span>
        </div>
        <div class="add-tag-input-wrapper" :class="{ active: showAddTag }">
          <input
            ref="newTagInput"
            v-model="newTagName"
            type="text"
            class="add-tag-input"
            placeholder="Tag..."
            @keydown.enter="finishAddTag"
            @keydown.esc="cancelAddTag"
            @blur="finishAddTag"
          />
        </div>
        <button class="add-tag-btn" @click="startAddTag">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="12" y1="5" x2="12" y2="19"></line>
            <line x1="5" y1="12" x2="19" y2="12"></line>
          </svg>
        </button>
      </div>
    </div>
    <div
      ref="clipboardList"
      class="clipboard-list"
      :class="{ dragging: isDragging }"
      @wheel.prevent="onWheel"
      @mousedown="onDragStart"
      @mousemove="onDragMove"
      @mouseup="onDragEnd"
      @mouseleave="onDragEnd"
    >
      <transition-group name="card">
        <div
          v-for="item in filterItems()"
          :key="item.id"
          class="card"
          :class="{ 'is-new': item.isNew, 'is-selected': selectedItemId === item.id }"
          @click="selectItem(item.id)"
          @dblclick="pasteItem(item)"
        >
          <div class="card-header" :style="{ backgroundColor: typeColors[item.content_type]?.header || '#4a90d9' }">
            <div class="header-left">
              <span class="type-label">{{ item.content_type === 'text' ? 'Text' : 'Image' }}</span>
              <span class="time-label">{{ formatTime(item.timestamp) }}</span>
            </div>
            <div class="header-right">
              <svg class="app-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <circle cx="12" cy="12" r="10"></circle>
                <path d="M8 12h8"></path>
              </svg>
            </div>
          </div>
          <div class="card-body">
            <template v-if="item.content_type === 'text'">
              <div class="text-preview">
                {{ truncateText(getTextContent(item)) }}
              </div>
            </template>
            <template v-else-if="item.content_type === 'image'">
              <div class="image-preview">
                <img
                  v-if="imageUrls[item.id]"
                  :src="imageUrls[item.id]"
                  alt="Clipboard image"
                  class="preview-image"
                />
                <div v-else class="image-loading">Loading...</div>
              </div>
            </template>
          </div>
          <div class="card-footer">
            <template v-if="item.content_type === 'text'">
              <span class="footer-info">{{ getCharCount(getTextContent(item)) }} characters</span>
            </template>
            <template v-else-if="item.content_type === 'image'">
              <span class="footer-info">{{ getImageDimensions(item) }}</span>
            </template>
          </div>
        </div>
      </transition-group>
      <div v-if="filterItems().length === 0" class="empty-state">
        <div class="empty-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
            <path d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2"></path>
          </svg>
        </div>
        <p v-if="searchQuery">No results found</p>
        <p v-else>No clipboard items yet</p>
        <span v-if="!searchQuery">Copy something to get started</span>
      </div>
    </div>
  </main>
</template>

<style scoped>
.top-bar {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 12px;
  z-index: 10;
  padding: 14px 24px;
}

.search-input-wrapper {
  display: flex;
  align-items: center;
  overflow: hidden;
  max-width: 32px;
  height: 32px;
  opacity: 1;
  background: transparent;
  border-radius: 8px;
  transition: max-width 0.3s cubic-bezier(0.4, 0, 0.2, 1), background 0.2s ease;
  cursor: pointer;
  flex-shrink: 0;
}

.search-input-wrapper:hover {
  background: rgba(0, 0, 0, 0.06);
}

.search-input-wrapper.active {
  max-width: 180px;
  background: rgba(0, 0, 0, 0.06);
  cursor: text;
}

.search-icon {
  width: 14px;
  height: 14px;
  color: rgba(0, 0, 0, 0.4);
  flex-shrink: 0;
  margin-left: 9px;
}

.search-input {
  width: 140px;
  padding: 6px 10px 6px 6px;
  border: none;
  outline: none;
  background: transparent;
  font-size: 13px;
  color: #1d1d1f;
  opacity: 0;
  transition: opacity 0.15s ease;
}

.search-input-wrapper.active .search-input {
  opacity: 1;
}

.search-input::placeholder {
  color: rgba(0, 0, 0, 0.35);
}

.tags-container {
  display: flex;
  align-items: center;
  gap: 6px;
}

.tag {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 0 10px;
  height: 32px;
  background: transparent;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s ease;
  white-space: nowrap;
  flex-shrink: 0;
}

.tag:hover {
  background: rgba(0, 0, 0, 0.06);
}

.tag.active {
  background: rgba(0, 0, 0, 0.08);
}

.tag-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
}

.tag-name {
  font-size: 12px;
  color: #1d1d1f;
  font-weight: 500;
}

.add-tag-input-wrapper {
  overflow: hidden;
}

.add-tag-input {
  width: 70px;
  padding: 0 8px;
  height: 32px;
  border: none;
  outline: none;
  font-size: 12px;
  color: #1d1d1f;
  background: rgba(0, 0, 0, 0.06);
  border-radius: 8px;
}

.add-tag-input::placeholder {
  color: rgba(0, 0, 0, 0.35);
}

.add-tag-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border: none;
  background: transparent;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s ease;
  flex-shrink: 0;
}

.add-tag-btn:hover {
  background: rgba(0, 0, 0, 0.06);
}

.add-tag-btn svg {
  width: 13px;
  height: 13px;
  color: #1d1d1f;
}

.add-tag-input-wrapper {
  overflow: hidden;
  max-width: 0;
  opacity: 0;
  transition: max-width 0.3s cubic-bezier(0.4, 0, 0.2, 1), opacity 0.2s ease;
  margin: 0;
}

.add-tag-input-wrapper.active {
  max-width: 90px;
  opacity: 1;
  margin: 0;
}

.clipboard-list {
  display: flex;
  flex-direction: row;
  gap: 16px;
  padding: 24px;
  padding-top: 72px;
  overflow-x: auto;
  overflow-y: hidden;
  height: 100%;
  align-items: flex-start;
  scrollbar-width: none;
  -ms-overflow-style: none;
  cursor: grab;
  user-select: none;
}

.clipboard-list::-webkit-scrollbar {
  display: none;
}

.clipboard-list.dragging {
  cursor: grabbing;
}

.card {
  flex: 0 0 calc(100vh - 96px);
  height: calc(100vh - 96px);
  background: #ffffff;
  border-radius: 12px;
  overflow: hidden;
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.1);
  outline: 3px solid transparent;
  outline-offset: -3px;
  transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1);
  cursor: pointer;
  display: flex;
  flex-direction: column;
  user-select: none;
}

.card * {
  user-select: none;
  -webkit-user-select: none;
}

.card.is-selected {
  outline-color: #007aff;
  box-shadow: 0 4px 20px rgba(0, 122, 255, 0.25);
}

.card:hover {
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.15);
}

.card.is-selected:hover {
  outline-color: #007aff;
  box-shadow: 0 8px 32px rgba(0, 122, 255, 0.2);
}

.card.is-new {
  animation: cardEnter 0.4s cubic-bezier(0.4, 0, 0.2, 1);
}

@keyframes cardEnter {
  0% {
    opacity: 0;
    transform: scale(0.8);
  }
  100% {
    opacity: 1;
    transform: scale(1);
  }
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 14px 14px;
  color: #ffffff;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 8px;
}

.type-label {
  font-size: 13px;
  font-weight: 600;
}

.time-label {
  font-size: 11px;
  opacity: 0.85;
}

.header-right {
  display: flex;
  align-items: center;
}

.app-icon {
  width: 20px;
  height: 20px;
  opacity: 0.9;
}

.card-body {
  flex: 1;
  padding: 12px;
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
}

.text-preview {
  font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", sans-serif;
  font-size: 13px;
  line-height: 1.5;
  color: #1d1d1f;
  white-space: pre-wrap;
  word-break: break-word;
  overflow: hidden;
  display: -webkit-box;
  -webkit-line-clamp: 12;
  line-clamp: 12;
  -webkit-box-orient: vertical;
  width: 100%;
}

.image-preview {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #f5f5f7;
  border-radius: 8px;
}

.preview-image {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
  border-radius: 4px;
  -webkit-user-drag: none;
  pointer-events: none;
}

.image-loading {
  font-size: 12px;
  color: #86868b;
}

.card-footer {
  padding: 10px 14px;
  border-top: 1px solid #f0f0f0;
  background: #fafafa;
}

.footer-info {
  font-size: 11px;
  color: #86868b;
}

.empty-state {
  width: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 60px 20px;
  color: #86868b;
}

.empty-icon {
  width: 64px;
  height: 64px;
  margin-bottom: 16px;
  opacity: 0.4;
}

.empty-icon svg {
  width: 100%;
  height: 100%;
}

.empty-state p {
  font-size: 16px;
  font-weight: 600;
  margin: 0 0 4px 0;
  color: #1d1d1f;
}

.empty-state span {
  font-size: 13px;
}

.card-enter-active {
  animation: cardEnter 0.4s cubic-bezier(0.4, 0, 0.2, 1);
}

.card-leave-active {
  animation: cardLeave 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

@keyframes cardLeave {
  0% {
    opacity: 1;
    transform: scale(1);
  }
  100% {
    opacity: 0;
    transform: scale(0.8);
  }
}

.card-move {
  transition: transform 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}
</style>

<style>
:root {
  font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", sans-serif;
  font-size: 16px;
  line-height: 1.5;
  font-weight: 400;
  color: #1d1d1f;
  background-color: transparent;
  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  -webkit-text-size-adjust: 100%;
}

html, body {
  margin: 0;
  padding: 0;
  background-color: transparent;
  height: 100%;
  overflow: hidden;
}

#app {
  background-color: transparent;
  height: 100%;
}

.container {
  margin: 0;
  display: flex;
  flex-direction: column;
  background-color: #ddddde;
  height: 100vh;
  overflow: hidden;
  border-top-left-radius: 16px;
  border-top-right-radius: 16px;
  position: relative;
}
</style>

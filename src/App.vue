<script setup>
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { computed, onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import ClipboardCard from "./components/ClipboardCard.vue";
import TopBar from "./components/TopBar.vue";
import { setLocale } from "./i18n";

const { t } = useI18n();

const clipboardItems = ref([]);
const maxItems = 30;
const imageUrls = ref({});
const clipboardList = ref(null);
const isDragging = ref(false);
const dragStartX = ref(0);
const scrollStartX = ref(0);
const activeTag = ref(1);
const selectedItemId = ref(null);

const tags = computed(() => [
  { id: 1, name: t("tags.clipboard"), isDefault: true },
  { id: 2, name: t("tags.work"), isDefault: false, color: "#4a90d9" },
  { id: 3, name: t("tags.personal"), isDefault: false, color: "#50c878" },
]);

onMounted(async () => {
  await loadHistory();

  const unlistenClipboard = await listen("clipboard-change", (event) => {
    const payload = event.payload;
    const item = {
      id: Date.now() + Math.random(),
      content_type: payload.content_type,
      content: payload.content,
      timestamp: payload.timestamp,
      is_pinned: false,
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

  const unlistenSettings = await listen("settings-changed", (event) => {
    const settings = event.payload;
    if (settings.locale) {
      setLocale(settings.locale);
    }
  });

  onUnmounted(() => {
    unlistenClipboard();
    unlistenSettings();
  });
});

async function loadHistory() {
  try {
    const items = await invoke("get_clipboard_history", {
      page: 0,
      pageSize: 50,
      tagId: null,
    });
    clipboardItems.value = items.map((item) => ({
      id: item.id,
      content_type: item.content_type,
      content: item.content,
      timestamp: item.created_at,
      is_pinned: item.is_pinned,
      isNew: false,
    }));
    for (const item of clipboardItems.value) {
      if (item.content_type === "image" && item.content?.Image) {
        const { width, height, rgba } = item.content.Image;
        getImageBlob(rgba, width, height, (blob) => {
          if (blob) {
            const url = URL.createObjectURL(blob);
            imageUrls.value[item.id] = url;
          }
        });
      }
    }
  } catch (e) {
    console.error("Failed to load history:", e);
  }
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

function onWheel(e) {
  if (!clipboardList.value) return;

  if (isDragging.value) {
    e.preventDefault();
    return;
  }

  if (e.deltaX !== 0) {
    e.preventDefault();
    clipboardList.value.scrollLeft += e.deltaX;
    return;
  }

  if (e.deltaY !== undefined && e.deltaY !== 0) {
    const isMouseWheel = Math.abs(e.deltaY) > 50 || e.maxTouchPoints === 0;
    if (isMouseWheel) {
      e.preventDefault();
      clipboardList.value.scrollLeft += e.deltaY;
    }
  }
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

function selectTag(tagId) {
  activeTag.value = tagId;
}

function addTag(tagName) {
  const colors = ["#4a90d9", "#50c878", "#e74c3c", "#9b59b6", "#f39c12", "#1abc9c"];
  const color = colors[tags.value.length % colors.length];
  tags.value.push({
    id: Date.now(),
    name: tagName,
    isDefault: false,
    color,
  });
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

const searchQuery = ref("");

function onSearch(query) {
  searchQuery.value = query;
}

async function openSettings() {
  try {
    await invoke("open_settings_window");
  } catch (e) {
    console.error("Failed to open settings:", e);
  }
}
</script>

<template>
  <main class="container">
    <TopBar
      :tags="tags"
      :active-tag="activeTag"
      @open-settings="openSettings"
      @select-tag="selectTag"
      @add-tag="addTag"
      @search="onSearch"
    />

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
        <ClipboardCard
          v-for="item in filterItems()"
          :key="item.id"
          :item="item"
          :image-url="imageUrls[item.id]"
          :is-selected="selectedItemId === item.id"
          @click="selectItem"
          @dblclick="pasteItem"
        />
      </transition-group>
      <div v-if="filterItems().length === 0" class="empty-state">
        <div class="empty-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
            <path d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2"></path>
          </svg>
        </div>
        <p v-if="searchQuery">{{ t('empty.noResults') }}</p>
        <p v-else>{{ t('empty.title') }}</p>
        <span v-if="!searchQuery">{{ t('empty.hint') }}</span>
      </div>
    </div>
  </main>
</template>

<style>
:root {
  font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", sans-serif;
  font-size: 16px;
  line-height: 1.5;
  color: #1a1a1a;
}

* {
  box-sizing: border-box;
}

html, body {
  margin: 0;
  padding: 0;
  width: 100%;
  height: 100%;
  overflow: hidden;
}

#app {
  width: 100%;
  height: 100%;
}
</style>

<style scoped>
.container {
  width: 100%;
  height: 100%;
  background: #b5b6b9;
  border-top-left-radius: 16px;
  border-top-right-radius: 16px;
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
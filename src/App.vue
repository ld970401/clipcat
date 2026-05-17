<script setup>
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { computed, onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import ClipboardCard from "./components/ClipboardCard.vue";
import TopBar from "./components/TopBar.vue";
import { setLocale } from "./i18n";

const { t } = useI18n();

const allItems = ref([]);
const imageUrls = ref({});
const clipboardList = ref(null);
const isDragging = ref(false);
const dragStartX = ref(0);
const scrollStartX = ref(0);
const activeTag = ref(1);
const selectedItemId = ref(null);
const tags = ref([]);
const searchQuery = ref("");
const isSearchMode = ref(false);
const isLoading = ref(false);
const activeMenuId = ref(null);
const topBarRef = ref(null);
const cardRefs = ref([]);
const maxItems = ref(50);

const clipboardItems = computed(() => {
  if (isSearchMode.value && searchQuery.value) {
    const q = searchQuery.value.toLowerCase();
    return allItems.value.filter((item) => {
      if (item.content_type === "text") {
        const text = item.content?.Text || "";
        return text.toLowerCase().includes(q);
      }
      return false;
    });
  }
  if (activeTag.value === 1) {
    return allItems.value;
  }
  return allItems.value.filter((item) =>
    item.tags.some((t) => t.id === activeTag.value)
  );
});

onMounted(async () => {
  const unlistenClipboard = await listen("clipboard-change", (event) => {
    const payload = event.payload;
    const item = {
      id: payload.id,
      content_type: payload.content_type,
      content: payload.content,
      timestamp: payload.timestamp,
      is_pinned: false,
      tags: payload.tags || [],
      isNew: true,
    };
    allItems.value.unshift(item);

    if (payload.content_type === "image" && payload.content?.Image?.thumbnail) {
      const { thumbnail } = payload.content.Image;
      const byteArray = new Uint8Array(thumbnail);
      const blob = new Blob([byteArray], { type: "image/png" });
      imageUrls.value[item.id] = URL.createObjectURL(blob);
    }

    if (allItems.value.length > maxItems.value) {
      const removed = allItems.value.pop();
      if (removed && imageUrls.value[removed.id]) {
        URL.revokeObjectURL(imageUrls.value[removed.id]);
        delete imageUrls.value[removed.id];
      }
    }
    setTimeout(() => {
      const newItem = allItems.value.find((i) => i.id === item.id);
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

  const unlistenWindowHiding = await listen("window-hiding", () => {
    if (topBarRef.value) {
      topBarRef.value.closeTagMenu();
    }
    cardRefs.value.forEach((cardRef) => {
      if (cardRef) {
        cardRef.closeContextMenu();
      }
    });
    activeMenuId.value = null;
  });

  const unlistenWindowShowing = await listen("window-showing", () => {
    loadHistory();
  });

  loadTags();
  loadHistory();

  function handleKeydown(e) {
    if (e.key === "Escape") {
      if (activeMenuId.value) {
        activeMenuId.value = null;
        cardRefs.value.forEach((cardRef) => {
          if (cardRef) cardRef.closeContextMenu();
        });
        if (topBarRef.value) topBarRef.value.closeTagMenu();
        return;
      }
      if (isSearchMode.value) {
        isSearchMode.value = false;
        searchQuery.value = "";
        return;
      }
    }

    if (e.key === "/" && !e.ctrlKey && !e.metaKey && !e.altKey) {
      const active = document.activeElement;
      if (active && (active.tagName === "INPUT" || active.tagName === "TEXTAREA")) return;
      e.preventDefault();
      isSearchMode.value = true;
    }

    if ((e.key === "Delete" || e.key === "Backspace") && selectedItemId.value) {
      const active = document.activeElement;
      if (active && (active.tagName === "INPUT" || active.tagName === "TEXTAREA")) return;
      deleteItem(selectedItemId.value);
    }
  }

  document.addEventListener("keydown", handleKeydown);

  onUnmounted(() => {
    unlistenClipboard();
    unlistenSettings();
    unlistenWindowHiding();
    unlistenWindowShowing();
    document.removeEventListener("keydown", handleKeydown);
  });
});

async function loadTags() {
  try {
    const backendTags = await invoke("get_all_tags");
    tags.value = backendTags.map((tag) => ({
      id: tag.id,
      name: tag.id === 1 ? t("tags.clipboard") : tag.name,
      color: tag.color,
      isDefault: tag.id === 1,
    }));
  } catch (e) {
    console.error("Failed to load tags:", e);
  }
}

async function loadHistory() {
  try {
    isLoading.value = true;
    for (const id of Object.keys(imageUrls.value)) {
      URL.revokeObjectURL(imageUrls.value[id]);
    }
    imageUrls.value = {};
    try {
      const settings = await invoke("get_settings");
      if (settings.retention_count && settings.retention_count > 0) {
        maxItems.value = settings.retention_count;
      }
    } catch (_) {}
    const items = await invoke("get_clipboard_history", {
      page: 0,
      pageSize: maxItems.value,
      tagId: null,
    });
    allItems.value = items.map((item) => ({
      id: item.id,
      content_type: item.content_type,
      content: item.content,
      timestamp: item.created_at,
      is_pinned: item.is_pinned,
      tags: item.tags || [],
      isNew: false,
    }));
    for (const item of allItems.value) {
      if (item.content_type === "image" && item.content?.Image?.thumbnail) {
        const { thumbnail } = item.content.Image;
        const byteArray = new Uint8Array(thumbnail);
        const blob = new Blob([byteArray], { type: "image/png" });
        imageUrls.value[item.id] = URL.createObjectURL(blob);
      }
    }
  } catch (e) {
    console.error("Failed to load history:", e);
  } finally {
    isLoading.value = false;
  }
}

function performSearch(keyword) {
  if (!keyword || !keyword.trim()) {
    isSearchMode.value = false;
    searchQuery.value = "";
    return;
  }
  isSearchMode.value = true;
  searchQuery.value = keyword.trim();
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
  isSearchMode.value = false;
  searchQuery.value = "";
}

async function addTag(tagName) {
  if (!tagName || !tagName.trim()) return;
  try {
    const newTag = await invoke("create_tag", {
      name: tagName.trim(),
      color: null,
    });
    tags.value.push({
      id: newTag.id,
      name: newTag.name,
      color: newTag.color,
      isDefault: false,
    });
  } catch (e) {
    console.error("Failed to create tag:", e);
  }
}

async function deleteTag(tagId) {
  if (tagId === 1) return;
  try {
    await invoke("delete_tag", { id: tagId });
    tags.value = tags.value.filter((t) => t.id !== tagId);
    allItems.value.forEach((item) => {
      item.tags = item.tags.filter((t) => t.id !== tagId);
    });
    if (activeTag.value === tagId) {
      activeTag.value = 1;
    }
  } catch (e) {
    console.error("Failed to delete tag:", e);
  }
}

async function updateTag({ id, name, color }) {
  if (id === 1) return;
  try {
    const tag = tags.value.find((t) => t.id === id);
    const newName = name !== undefined ? name : tag?.name;
    const newColor = color !== undefined ? color : tag?.color;
    await invoke("update_tag", { id, name: newName, color: newColor });
    if (tag) {
      if (name !== undefined) tag.name = name;
      if (color !== undefined) tag.color = color;
    }
  } catch (e) {
    console.error("Failed to update tag:", e);
  }
}

function selectItem(itemId) {
  selectedItemId.value = selectedItemId.value === itemId ? null : itemId;
}

async function pasteItem(item) {
  try {
    const isFirstItem = allItems.value.length > 0 && allItems.value[0].id === item.id;
    const result = await invoke("paste_item", { id: item.id, moveToTop: !isFirstItem });
    if (result) {
      const oldImageUrl = imageUrls.value[item.id];
      if (oldImageUrl) {
        URL.revokeObjectURL(oldImageUrl);
        delete imageUrls.value[item.id];
      }

      const newItem = {
        id: result.id,
        content_type: result.content_type,
        content: result.content,
        timestamp: result.created_at,
        is_pinned: result.is_pinned,
        tags: result.tags || [],
        isNew: false,
      };

      if (!isFirstItem) {
        allItems.value = allItems.value.filter((i) => i.id !== item.id);
        allItems.value.unshift(newItem);
      } else {
        const idx = allItems.value.findIndex((i) => i.id === item.id);
        if (idx !== -1) {
          allItems.value[idx] = newItem;
        }
      }

      if (newItem.content_type === "image" && newItem.content?.Image?.thumbnail) {
        const { thumbnail } = newItem.content.Image;
        const byteArray = new Uint8Array(thumbnail);
        const blob = new Blob([byteArray], { type: "image/png" });
        imageUrls.value[newItem.id] = URL.createObjectURL(blob);
      }
    }
  } catch (e) {
    console.error("Failed to paste item:", e);
  }
}

async function copyItem(item) {
  try {
    await invoke("copy_to_clipboard", { id: item.id });
  } catch (e) {
    console.error("Failed to copy item:", e);
  }
}

async function togglePinItem(itemId) {
  try {
    await invoke("toggle_pin", { id: itemId });
    const item = allItems.value.find((i) => i.id === itemId);
    if (item) {
      item.is_pinned = !item.is_pinned;
      sortItems();
    }
  } catch (e) {
    console.error("Failed to toggle pin:", e);
  }
}

function sortItems() {
  allItems.value.sort((a, b) => {
    if (a.is_pinned !== b.is_pinned) return b.is_pinned ? 1 : -1;
    return b.timestamp - a.timestamp;
  });
}

async function deleteItem(itemId) {
  try {
    await invoke("delete_clipboard_item", { id: itemId });
    const imageUrl = imageUrls.value[itemId];
    if (imageUrl) {
      URL.revokeObjectURL(imageUrl);
      delete imageUrls.value[itemId];
    }
    allItems.value = allItems.value.filter((i) => i.id !== itemId);
  } catch (e) {
    console.error("Failed to delete item:", e);
  }
}

async function updateItemTags({ id, tagIds }) {
  try {
    await invoke("update_record_tags", { id, tagIds: tagIds });
    const item = allItems.value.find((i) => i.id === id);
    if (item) {
      item.tags = tags.value.filter((t) => tagIds.includes(t.id));
    }
  } catch (e) {
    console.error("Failed to update item tags:", e);
  }
}

function updateActiveMenu(menuId) {
  if (menuId !== null) {
    if (topBarRef.value) {
      topBarRef.value.closeTagMenu();
    }
  }
  activeMenuId.value = menuId;
}

function closeAllCardMenus() {
  activeMenuId.value = null;
}

function getTextContent(item) {
  if (!item.content) return "";
  if (typeof item.content === "string") return item.content;
  if (item.content.Text) return item.content.Text;
  return "";
}

const searchInputKeydown = (event) => {
  if (event.key === "Enter") {
    performSearch(searchQuery.value);
  }
};

function onSearch(query) {
  if (!query || !query.trim()) {
    searchQuery.value = "";
    isSearchMode.value = false;
    return;
  }
  searchQuery.value = query;
  performSearch(query);
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
      ref="topBarRef"
      :tags="tags"
      :active-tag="activeTag"
      @open-settings="openSettings"
      @select-tag="selectTag"
      @add-tag="addTag"
      @delete-tag="deleteTag"
      @update-tag="updateTag"
      @search="onSearch"
      @menu-open="closeAllCardMenus"
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
      <ClipboardCard
        v-for="item in clipboardItems"
        :key="item.id"
        :ref="(el) => {
          if (el) {
            const idx = clipboardItems.findIndex((i) => i.id === item.id);
            cardRefs[idx] = el;
          }
        }"
        :item="item"
        :image-url="imageUrls[item.id]"
        :is-selected="selectedItemId === item.id"
        :tags="tags"
        :menu-id="item.id"
        :active-menu-id="activeMenuId"
        @click="selectItem"
        @dblclick="pasteItem"
        @copy="copyItem"
        @toggle-pin="togglePinItem"
        @delete="deleteItem"
        @update-tags="updateItemTags"
        @update-active-menu="updateActiveMenu"
      />
      <div v-if="clipboardItems.length === 0" class="empty-state">
        <div class="empty-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
            <path d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2"></path>
          </svg>
        </div>
        <p v-if="isSearchMode">{{ t('empty.noResults') }}</p>
        <p v-else>{{ t('empty.title') }}</p>
        <span v-if="!isSearchMode">{{ t('empty.hint') }}</span>
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
  padding: 60px 24px 24px;
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
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 60px 20px;
  color: #86868b;
  pointer-events: none;
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
</style>

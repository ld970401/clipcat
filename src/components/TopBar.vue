<script setup>
import { computed, nextTick, onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";

const { t } = useI18n();

const props = defineProps({
  tags: {
    type: Array,
    default: () => [],
  },
  activeTag: {
    type: Number,
    default: 1,
  },
});

const emit = defineEmits(["openSettings", "selectTag", "addTag", "search", "deleteTag", "updateTag", "menu-open", "menu-close"]);

const sortedTags = computed(() => {
  return [...props.tags].sort((a, b) => {
    if (a.id === 1) return -1;
    if (b.id === 1) return 1;
    return 0;
  });
});

const searchQuery = ref("");
const isSearchFocused = ref(false);
const searchInput = ref(null);
const showAddTag = ref(false);
const newTagName = ref("");
const newTagInput = ref(null);

const showTagMenu = ref(false);
const tagMenuX = ref(0);
const tagMenuY = ref(0);
const activeTagMenuId = ref(null);
const renameValue = ref("");
const renamingTagId = ref(null);

const tagColors = [
  "#ff3b30",
  "#ff9500",
  "#ffcc00",
  "#34c759",
  "#007aff",
  "#af52de",
  "#ff2d55",
  "#8e8e93",
];

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
  emit("search", searchQuery.value);
}

function selectTag(tagId) {
  emit("selectTag", tagId);
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
    emit("addTag", newTagName.value.trim());
  }
  newTagName.value = "";
  showAddTag.value = false;
}

function cancelAddTag() {
  newTagName.value = "";
  showAddTag.value = false;
}

function handleAddTagInputBlur(event) {
  if (!event.relatedTarget || !event.relatedTarget.closest('.add-tag-btn')) {
    finishAddTag();
  }
}

function openSettings() {
  emit("openSettings");
}

function handleTagContextMenu(event, tag) {
  if (tag.id === 1) return;
  event.preventDefault();
  event.stopPropagation();
  showTagMenu.value = true;
  const rect = event.currentTarget.getBoundingClientRect();
  tagMenuX.value = rect.left;
  tagMenuY.value = rect.bottom + 4;
  activeTagMenuId.value = tag.id;
  emit("menu-open");
}

function closeTagMenu() {
  showTagMenu.value = false;
  activeTagMenuId.value = null;
  renamingTagId.value = null;
  renameValue.value = "";
  emit("menu-close");
}

function handleClickOutside(event) {
  const menu = document.querySelector('.tag-context-menu');
  if (menu && !menu.contains(event.target)) {
    closeTagMenu();
  }
}

function startRename() {
  const tag = props.tags.find((t) => t.id === activeTagMenuId.value);
  if (tag) {
    renamingTagId.value = tag.id;
    renameValue.value = tag.name;
    showTagMenu.value = false;
    nextTick(() => {
      const renameEl = document.querySelector(`.rename-tag-input[data-tag-id="${tag.id}"]`);
      if (renameEl) {
        renameEl.focus();
        renameEl.select();
      }
    });
  }
}

function finishRename() {
  if (renameValue.value.trim()) {
    emit("updateTag", { id: renamingTagId.value, name: renameValue.value.trim() });
  }
  renamingTagId.value = null;
  renameValue.value = "";
}

function cancelRename() {
  renamingTagId.value = null;
  renameValue.value = "";
}

function deleteTag() {
  if (activeTagMenuId.value === 1) return;
  emit("deleteTag", activeTagMenuId.value);
  closeTagMenu();
}

function selectColor(color) {
  emit("updateTag", { id: activeTagMenuId.value, color });
}

function getCurrentTagColor() {
  const tag = props.tags.find((t) => t.id === activeTagMenuId.value);
  return tag?.color || "";
}

onMounted(() => {
  document.addEventListener("click", handleClickOutside);
});

onUnmounted(() => {
  document.removeEventListener("click", handleClickOutside);
});

defineExpose({
  searchQuery,
  closeTagMenu,
});
</script>

<template>
  <div class="top-bar">
    <button class="settings-btn" @click="openSettings">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <circle cx="12" cy="12" r="3"></circle>
        <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path>
      </svg>
    </button>
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
        :placeholder="t('search.placeholder')"
        @blur="blurSearch"
        @keydown.enter="blurSearch"
        @keydown.esc="blurSearch"
      />
    </div>
    <div class="tags-container">
      <template v-for="tag in sortedTags" :key="tag.id">
        <div
          v-if="renamingTagId !== tag.id"
          class="tag"
          :class="{ active: activeTag === tag.id }"
          @click="selectTag(tag.id)"
          @contextmenu.prevent="handleTagContextMenu($event, tag)"
        >
          <span v-if="!tag.isDefault" class="tag-dot" :style="{ backgroundColor: tag.color }"></span>
          <span class="tag-name">{{ tag.name }}</span>
        </div>
        <div
          v-else
          class="tag rename-tag-wrapper"
        >
          <input
            class="rename-tag-input"
            :data-tag-id="tag.id"
            v-model="renameValue"
            type="text"
            @keydown.enter="finishRename"
            @keydown.esc="cancelRename"
            @blur="finishRename"
          />
        </div>
      </template>
      <div class="add-tag-input-wrapper" :class="{ active: showAddTag }">
        <input
          ref="newTagInput"
          v-model="newTagName"
          type="text"
          class="add-tag-input"
          placeholder="Tag..."
          @keydown.enter="finishAddTag"
          @keydown.esc="cancelAddTag"
          @blur="handleAddTagInputBlur"
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

  <Teleport to="body">
    <div
      v-if="showTagMenu"
      class="tag-context-menu"
      :style="{ left: tagMenuX + 'px', top: tagMenuY + 'px' }"
    >
      <div class="menu-item" @click.stop="startRename">
        <span>{{ t("tagMenu.rename") }}</span>
      </div>
      <div class="menu-divider"></div>
      <div
        class="menu-item danger"
        @click.stop="deleteTag"
      >
        <span>{{ t("tagMenu.delete") }}</span>
      </div>
      <div class="menu-divider"></div>
      <div class="color-picker">
        <div
          v-for="color in tagColors"
          :key="color"
          class="color-dot"
          :class="{ selected: getCurrentTagColor() === color }"
          :style="{ backgroundColor: color }"
          @click.stop="selectColor(color)"
        ></div>
      </div>
    </div>
  </Teleport>
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

.settings-btn {
  position: absolute;
  right: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  background: transparent;
  border: none;
  border-radius: 8px;
  cursor: pointer;
  color: rgba(0, 0, 0, 0.4);
  transition: all 0.2s ease;
  flex-shrink: 0;
}

.settings-btn:hover {
  background: rgba(0, 0, 0, 0.05);
  color: rgba(0, 0, 0, 0.6);
}

.settings-btn svg {
  width: 14px;
  height: 14px;
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
  user-select: none;
  -webkit-user-select: none;
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

.tag-context-menu {
  position: fixed;
  background: #ffffff;
  border-radius: 10px;
  box-shadow: 0 8px 30px rgba(0, 0, 0, 0.15);
  min-width: 160px;
  padding: 6px;
  z-index: 10000;
}

.tag-context-menu .menu-item {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 32px;
  padding: 0 14px;
  cursor: pointer;
  font-size: 15px;
  color: #1d1d1f;
  position: relative;
  border-radius: 6px;
}

.tag-context-menu .menu-item:hover {
  background: #f2f2f2;
}

.tag-context-menu .menu-item.danger {
  color: #ff3b30;
}

.tag-context-menu .menu-item.danger:hover {
  background: #fff0f0;
}

.tag-context-menu .menu-divider {
  height: 1px;
  background: #e5e5e5;
  margin: 4px 8px;
}

.rename-input-wrapper {
  padding: 4px 8px;
}

.rename-input {
  width: 100%;
  height: 32px;
  padding: 0 10px;
  border: 1px solid #007aff;
  border-radius: 6px;
  outline: none;
  font-size: 14px;
  color: #1d1d1f;
  background: #ffffff;
}

.color-picker {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  padding: 6px 8px;
  justify-content: center;
}

.color-dot {
  width: 18px;
  height: 18px;
  border-radius: 50%;
  cursor: pointer;
  transition: transform 0.15s ease;
  border: 2px solid transparent;
}

.color-dot:hover {
  transform: scale(1.15);
}

.color-dot.selected {
  border-color: #1d1d1f;
}

.rename-tag-wrapper {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 0 10px;
  height: 32px;
  background: rgba(0, 0, 0, 0.06);
  border-radius: 8px;
  flex-shrink: 0;
}

.rename-tag-input {
  width: 70px;
  padding: 0;
  height: 32px;
  border: none;
  outline: none;
  font-size: 12px;
  color: #1d1d1f;
  background: transparent;
  font-weight: 500;
}

.rename-tag-input::placeholder {
  color: rgba(0, 0, 0, 0.35);
}
</style>
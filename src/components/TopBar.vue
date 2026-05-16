<script setup>
import { nextTick, ref } from "vue";
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

const emit = defineEmits(["openSettings", "selectTag", "addTag"]);

const searchQuery = ref("");
const isSearchFocused = ref(false);
const searchInput = ref(null);
const showAddTag = ref(false);
const newTagName = ref("");
const newTagInput = ref(null);

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

function openSettings() {
  emit("openSettings");
}

defineExpose({
  searchQuery,
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
</style>
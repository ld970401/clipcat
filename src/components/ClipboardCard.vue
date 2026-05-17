<script setup>
import { onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";

const { t } = useI18n();

const props = defineProps({
  item: {
    type: Object,
    required: true,
  },
  imageUrl: {
    type: String,
    default: null,
  },
  isSelected: {
    type: Boolean,
    default: false,
  },
  tags: {
    type: Array,
    default: () => [],
  },
  menuId: {
    type: [String, Number],
    default: null,
  },
  activeMenuId: {
    type: [String, Number],
    default: null,
  },
});

const emit = defineEmits(["click", "dblclick", "copy", "toggle-pin", "delete", "update-tags", "update-active-menu"]);

const typeColors = {
  text: { header: "#4a90d9", icon: "#ffffff" },
  image: { header: "#50c878", icon: "#ffffff" },
};

const showContextMenu = ref(false);
const contextMenuX = ref(0);
const contextMenuY = ref(0);

watch(() => props.activeMenuId, (newId) => {
  if (newId !== props.menuId) {
    showContextMenu.value = false;
  }
});
const showPinSubmenu = ref(false);

function getTextContent(item) {
  if (!item.content) return "";
  if (typeof item.content === "string") return item.content;
  if (item.content.Text) return item.content.Text;
  return "";
}

function truncateText(text, maxChars = 200) {
  if (!text) return "";
  if (text.length <= maxChars) return text;
  return text.slice(0, maxChars) + "...";
}

function formatTime(timestamp) {
  const date = new Date(timestamp);
  const now = new Date();
  const diff = now - date;

  if (diff < 60000) return t("card.justNow");
  if (diff < 3600000) return t("card.minutesAgo", { n: Math.floor(diff / 60000) });
  if (diff < 86400000) return t("card.hoursAgo", { n: Math.floor(diff / 3600000) });

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

function handleClick() {
  emit("click", props.item.id);
}

function handleDblClick() {
  emit("dblclick", props.item);
}

function handleContextMenu(event) {
  event.preventDefault();
  event.stopPropagation();
  showContextMenu.value = true;
  showPinSubmenu.value = false;
  contextMenuX.value = event.clientX;
  contextMenuY.value = event.clientY;
  emit("update-active-menu", props.menuId);
}

function handleClickOutside(event) {
  const menu = document.querySelector('.context-menu');
  if (menu && !menu.contains(event.target)) {
    showContextMenu.value = false;
    showPinSubmenu.value = false;
    emit("update-active-menu", null);
  }
}

function handleCopy() {
  showContextMenu.value = false;
  emit("copy", props.item);
}

function handleTogglePin() {
  showContextMenu.value = false;
  emit("toggle-pin", props.item.id);
}

function handleDelete() {
  showContextMenu.value = false;
  emit("delete", props.item.id);
}

function handlePinSubmenuEnter() {
  showPinSubmenu.value = true;
}

function handlePinSubmenuLeave() {
  showPinSubmenu.value = false;
}

function handleSelectTag(tagId) {
  showContextMenu.value = false;
  showPinSubmenu.value = false;
  const currentTagIds = props.item.tags.map((t) => t.id);
  let newTagIds;
  if (currentTagIds.includes(tagId)) {
    newTagIds = currentTagIds.filter((id) => id !== tagId);
  } else {
    newTagIds = [...currentTagIds, tagId];
  }
  emit("update-tags", { id: props.item.id, tagIds: newTagIds });
}

function hasTag(tagId) {
  return props.item.tags.some((t) => t.id === tagId);
}

function getCustomTags() {
  return props.tags.filter((tag) => tag.id !== 1);
}


function closeContextMenu() {
  showContextMenu.value = false;
  showPinSubmenu.value = false;
}

onMounted(() => {
  document.addEventListener("click", handleClickOutside);
});

onUnmounted(() => {
  document.removeEventListener("click", handleClickOutside);
});

defineExpose({
  closeContextMenu,
});
</script>

<template>
  <div
    class="card"
    :class="{ 'is-new': item.isNew, 'is-selected': isSelected }"
    @click="handleClick"
    @dblclick="handleDblClick"
    @contextmenu.prevent="handleContextMenu"
  >
    <div class="card-header" :style="{ backgroundColor: typeColors[item.content_type]?.header || '#4a90d9' }">
      <div class="header-left">
        <span class="type-label">{{ item.content_type === 'text' ? t('card.text') : t('card.image') }}</span>
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
            v-if="imageUrl"
            :src="imageUrl"
            alt="Clipboard image"
            class="preview-image"
            draggable="false"
          />
          <div v-else class="image-loading">{{ t('card.loading') }}</div>
        </div>
      </template>
    </div>
    <div class="card-footer">
      <template v-if="item.content_type === 'text'">
        <span class="footer-info">{{ getCharCount(getTextContent(item)) }} {{ t('card.chars') }}</span>
      </template>
      <template v-else-if="item.content_type === 'image'">
        <span class="footer-info">{{ t('card.size') }}: {{ getImageDimensions(item) }}</span>
      </template>
    </div>

    <Teleport to="body">
      <div
        v-if="showContextMenu"
        class="context-menu"
        :style="{ left: contextMenuX + 'px', top: contextMenuY + 'px' }"
      >
        <div class="menu-item" @click.stop="handleCopy">
          <span>{{ t("contextMenu.copy") }}</span>
        </div>
        <template v-if="getCustomTags().length > 0">
          <div
            class="menu-item has-submenu"
            @mouseenter="handlePinSubmenuEnter"
            @mouseleave="handlePinSubmenuLeave"
          >
            <span>{{ t("contextMenu.tags") }}</span>
            <span class="submenu-arrow"><svg viewBox="0 0 1024 1024" version="1.1" xmlns="http://www.w3.org/2000/svg" width="16" height="16"><path d="M318.57 223.95l322.99 322.99c21.87 21.87 57.33 21.87 79.2 0 21.87-21.87 21.87-57.33 0-79.2l-323-322.99c-21.87-21.87-57.33-21.87-79.2 0-21.86 21.87-21.86 57.33 0.01 79.2z" fill="currentColor"></path><path d="M729.75 555.95L406.76 878.93c-21.87 21.87-57.33 21.87-79.2 0-21.87-21.87-21.87-57.33 0-79.2l322.99-322.99c21.87-21.87 57.33-21.87 79.2 0 21.87 21.88 21.87 57.34 0 79.21z" fill="currentColor"></path></svg></span>
            <div v-if="showPinSubmenu" class="submenu">
              <div
                v-for="tag in getCustomTags()"
                :key="tag.id"
                class="menu-item"
                :class="{ active: hasTag(tag.id) }"
                @click.stop="handleSelectTag(tag.id)"
              >
                <span class="tag-dot" :style="{ backgroundColor: tag.color }"></span>
                <span>{{ tag.name }}</span>
                <span v-if="hasTag(tag.id)" class="check-mark">✓</span>
              </div>
            </div>
          </div>
          <div class="menu-divider"></div>
        </template>
        <div class="menu-item danger" @click.stop="handleDelete">
          <span>{{ t("contextMenu.delete") }}</span>
        </div>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.card {
  flex: 0 0 calc(100vh - 96px);
  height: calc(100vh - 84px);
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
  width: 18px;
  height: 18px;
}

.card-body {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.text-preview {
  padding: 14px;
  font-size: 14px;
  line-height: 1.5;
  color: #333;
  overflow: hidden;
  word-break: break-word;
  white-space: pre-wrap;
}

.image-preview {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #f5f5f5;
  padding: 14px;
}

.preview-image {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
  border-radius: 8px;
  user-select: none;
  -webkit-user-drag: none;
}

.image-loading {
  color: #999;
  font-size: 14px;
}

.card-footer {
  padding: 10px 14px;
  background: #fafafa;
  border-top: 1px solid #eee;
}

.footer-info {
  font-size: 11px;
  color: #999;
}

.context-menu {
  position: fixed;
  background: #ffffff;
  border-radius: 10px;
  box-shadow: 0 8px 30px rgba(0, 0, 0, 0.15);
  min-width: 160px;
  padding: 6px;
  z-index: 10000;
}

.menu-item {
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

.menu-item:hover {
  background: #f2f2f2;
}

.menu-item.danger {
  color: #ff3b30;
}

.menu-item.danger:hover {
  background: #fff0f0;
}

.submenu-arrow {
  margin-top: 5px;
  margin-left: auto;
  font-size: 12px;
  color: #999;
}

.menu-divider {
  height: 1px;
  background: #e5e5e5;
  margin: 4px 8px;
}

.has-submenu {
  position: relative;
}

.submenu {
  position: absolute;
  left: 100%;
  top: 0;
  background: #ffffff;
  border-radius: 10px;
  box-shadow: 0 8px 30px rgba(0, 0, 0, 0.15);
  min-width: 140px;
  padding: 6px;
  margin-left: 4px;
}

.submenu .menu-item.active {
  background: #f2f2f2;
}

.tag-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.check-mark {
  margin-left: auto;
  color: #007aff;
  font-weight: bold;
}
</style>
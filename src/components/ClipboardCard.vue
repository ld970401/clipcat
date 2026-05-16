<script setup>
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
});

const emit = defineEmits(["click", "dblclick"]);

const typeColors = {
  text: { header: "#4a90d9", icon: "#ffffff" },
  image: { header: "#50c878", icon: "#ffffff" },
};

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
</script>

<template>
  <div
    class="card"
    :class="{ 'is-new': item.isNew, 'is-selected': isSelected }"
    @click="handleClick"
    @dblclick="handleDblClick"
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
  </div>
</template>

<style scoped>
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
</style>
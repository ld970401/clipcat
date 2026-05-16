import { createI18n } from "vue-i18n";
import zh from "./locales/zh.json";
import en from "./locales/en.json";

const i18n = createI18n({
  legacy: false,
  locale: localStorage.getItem("locale") || (navigator.language.startsWith("zh") ? "zh" : "en"),
  fallbackLocale: "en",
  messages: {
    zh,
    en,
  },
});

export function setLocale(locale) {
  i18n.global.locale.value = locale;
  localStorage.setItem("locale", locale);
}

export function getLocale() {
  return i18n.global.locale.value;
}

export default i18n;
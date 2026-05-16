import { createApp } from "vue";
import SettingsApp from "./SettingsApp.vue";
import i18n from "./i18n";

createApp(SettingsApp).use(i18n).mount("#app");
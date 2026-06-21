import i18n from "i18next";
import { initReactI18next } from "react-i18next";

const resources = {
  en: {
    translation: {
      Dashboard: "Dashboard",
      Policies: "Policies",
      Connectors: "Connectors",
      Settings: "Settings",
      Simulator: "Simulator",
      "Decision Logs": "Decision Logs",
      Language: "Language",
    },
  },
  th: {
    translation: {
      Dashboard: "หน้าหลัก",
      Policies: "นโยบาย",
      Connectors: "การเชื่อมต่อ",
      Settings: "การตั้งค่า",
      Simulator: "จำลองสถานการณ์",
      "Decision Logs": "บันทึกการตัดสินใจ",
      Language: "ภาษา",
    },
  },
};

i18n.use(initReactI18next).init({
  resources,
  lng: localStorage.getItem("i18nextLng") || "en",
  fallbackLng: "en",
  interpolation: {
    escapeValue: false,
  },
});

export default i18n;

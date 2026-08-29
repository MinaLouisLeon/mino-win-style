import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";

import ar from "./locales/ar.json";
import en from "./locales/en.json";

export type Lang = "ar" | "en";

type Dict = Record<string, string>;

const dicts: Record<Lang, Dict> = { ar, en };

interface I18nValue {
  lang: Lang;
  dir: "rtl" | "ltr";
  setLang: (lang: Lang) => void;
  /** `t("plan.apply")`, or `t("history.changes", { n: 3 })` for counted text. */
  t: (key: string, vars?: Record<string, string | number>) => string;
  /** Singular/plural pair, since Arabic and English disagree about where 1 ends. */
  tCount: (key: string, n: number) => string;
  /** Translate if we have it, otherwise show what the Rust side sent. */
  tOr: (key: string, fallback: string) => string;
}

const I18nContext = createContext<I18nValue | null>(null);

/** What the app opens in before anyone has chosen. English, deliberately:
 *  Arabic is a choice the user makes, not one the app makes for them. */
const DEFAULT_LANG: Lang = "en";

function stored(): Lang {
  try {
    const saved = localStorage.getItem("mws-lang");
    if (saved === "ar" || saved === "en") return saved;
  } catch {
    // Private windows and locked-down profiles throw here; the default is fine.
  }
  return DEFAULT_LANG;
}

export function I18nProvider({ children }: { children: ReactNode }) {
  const [lang, setLangState] = useState<Lang>(stored);
  const dir = lang === "ar" ? "rtl" : "ltr";

  useEffect(() => {
    document.documentElement.lang = lang;
    document.documentElement.dir = dir;
  }, [lang, dir]);

  const setLang = useCallback((next: Lang) => {
    setLangState(next);
    try {
      localStorage.setItem("mws-lang", next);
    } catch {
      // Not being able to remember the choice is not worth an error.
    }
  }, []);

  const t = useCallback(
    (key: string, vars?: Record<string, string | number>) => {
      const dict = dicts[lang];
      let text = dict[key] ?? dicts.en[key] ?? key;
      if (vars) {
        for (const [name, value] of Object.entries(vars)) {
          text = text.replaceAll(`{${name}}`, String(value));
        }
      }
      return text;
    },
    [lang],
  );

  const tCount = useCallback(
    (key: string, n: number) => t(n === 1 ? key : `${key}Plural`, { n }),
    [t],
  );

  const tOr = useCallback(
    (key: string, fallback: string) => {
      const text = t(key);
      return text === key ? fallback : text;
    },
    [t],
  );

  const value = useMemo<I18nValue>(
    () => ({ lang, dir, setLang, t, tCount, tOr }),
    [lang, dir, setLang, t, tCount, tOr],
  );

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n(): I18nValue {
  const value = useContext(I18nContext);
  if (!value) throw new Error("useI18n must be used inside <I18nProvider>");
  return value;
}

/**
 * Names and option labels for a setting. Falls back to the raw id so a tweak
 * added in Rust before its translation exists still shows something.
 */
export function useTweakText() {
  const { t } = useI18n();
  return {
    name: (id: string) => {
      const key = `tweak.${id}.name`;
      const text = t(key);
      return text === key ? id : text;
    },
    desc: (id: string) => {
      const key = `tweak.${id}.desc`;
      const text = t(key);
      return text === key ? null : text;
    },
    choice: (id: string, choice: string) => {
      const key = `tweak.${id}.choice.${choice}`;
      const text = t(key);
      return text === key ? choice : text;
    },
  };
}

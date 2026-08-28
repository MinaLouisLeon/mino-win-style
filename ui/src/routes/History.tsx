import type { JournalEntry } from "../lib/api";
import { useI18n } from "../i18n";

interface Props {
  entries: JournalEntry[];
  busy: boolean;
  onRevert: (id: string) => void;
}

export function History({ entries, busy, onRevert }: Props) {
  const { t, tCount, lang } = useI18n();
  const when = new Intl.DateTimeFormat(lang === "ar" ? "ar-EG" : "en-GB", {
    dateStyle: "medium",
    timeStyle: "short",
  });

  return (
    <>
      <header className="page">
        <h1>{t("history.title")}</h1>
        <p className="muted small" dir="ltr">
          {t("history.safeRestore")}
        </p>
      </header>

      {entries.length === 0 && <p className="muted">{t("history.empty")}</p>}

      <ol className="timeline">
        {entries.map((entry) => (
          <li key={entry.id} className={`timeline__item timeline__item--${entry.status}`}>
            <div className="timeline__when">{when.format(new Date(entry.when))}</div>
            <div className="timeline__body">
              <strong>{entry.label}</strong>
              <p className="muted small">
                {t(`history.status.${entry.status}`)} ·{" "}
                {tCount("history.changes", entry.changes.length)}
              </p>
            </div>
            <button
              type="button"
              className="btn"
              disabled={busy || entry.status === "reverted" || entry.changes.length === 0}
              onClick={() => onRevert(entry.id)}
            >
              {t("history.revert")}
            </button>
          </li>
        ))}
      </ol>
    </>
  );
}

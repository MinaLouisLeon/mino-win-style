import { TweakRow } from "../components/TweakRow";
import { useI18n } from "../i18n";
import type { Category as Cat, TweakState, Value } from "../lib/api";

interface Props {
  category: Cat;
  tweaks: TweakState[];
  pending: Record<string, Value>;
  onChange: (id: string, value: Value) => void;
}

export function Category({ category, tweaks, pending, onChange }: Props) {
  const { t } = useI18n();
  const rows = tweaks.filter((tweak) => tweak.category === category);

  // Settings Windows no longer honours sink to the bottom rather than
  // disappearing: knowing a thing is gone is more useful than not finding it.
  const ordered = [
    ...rows.filter((row) => row.support.level !== "unsupported"),
    ...rows.filter((row) => row.support.level === "unsupported"),
  ];

  return (
    <>
      <header className="page">
        <h1>{t(`nav.${category}`)}</h1>
      </header>

      <section className="rows">
        {ordered.map((row) => (
          <TweakRow key={row.id} state={row} pending={pending[row.id]} onChange={onChange} />
        ))}
      </section>
    </>
  );
}

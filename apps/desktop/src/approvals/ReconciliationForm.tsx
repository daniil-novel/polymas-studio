import { useId } from 'react';
export interface ReconciliationDraft {
  outcome: '' | 'succeeded' | 'failed';
  note: string;
}
export function ReconciliationForm({
  draft,
  disabled,
  onChange,
  onSubmit,
}: {
  draft: ReconciliationDraft;
  disabled: boolean;
  onChange: (draft: ReconciliationDraft) => void;
  onSubmit: () => void;
}) {
  const id = useId();
  const bytes = new TextEncoder().encode(draft.note).length;
  const valid = draft.outcome !== '' && draft.note.trim().length >= 10 && bytes <= 2000;
  return (
    <form
      className="approval-reconcile"
      onSubmit={(event) => {
        event.preventDefault();
        if (valid && !disabled) onSubmit();
      }}
    >
      <h3>Сверка неизвестного исхода</h3>
      <p>
        Проверьте фактическое состояние ресурса и сохраните наблюдаемый результат. К задаче можно
        вернуться после сверки.
      </p>
      <fieldset disabled={disabled}>
        <label htmlFor={`${id}-outcome`}>Проверенный исход</label>
        <select
          id={`${id}-outcome`}
          value={draft.outcome}
          onChange={(event) =>
            onChange({ ...draft, outcome: event.target.value as ReconciliationDraft['outcome'] })
          }
        >
          <option value="">Выберите результат проверки</option>
          <option value="succeeded">Изменение выполнено</option>
          <option value="failed">Не выполнено или завершилось ошибкой</option>
        </select>
        <label htmlFor={`${id}-note`}>Что вы проверили</label>
        <textarea
          id={`${id}-note`}
          rows={4}
          maxLength={1000}
          value={draft.note}
          onChange={(event) => onChange({ ...draft, note: event.target.value })}
          placeholder="Например: проверены документ, его версия и контрольная сумма в источнике"
        />
        {bytes > 2000 && <p className="form-error">Описание слишком длинное. Сократите его.</p>}
        <button type="submit" disabled={!valid || disabled}>
          Сохранить результат сверки
        </button>
      </fieldset>
    </form>
  );
}

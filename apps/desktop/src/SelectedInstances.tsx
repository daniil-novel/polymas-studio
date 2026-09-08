import { useState } from 'react';

export function SelectedInstances({
  items,
}: {
  items: { id: string; name: string; detail: string }[];
}) {
  const [query, setQuery] = useState('');
  const [page, setPage] = useState(0);
  const needle = query.trim().toLocaleLowerCase('ru');
  const filtered = items.filter((item) =>
    `${item.name} ${item.id}`.toLocaleLowerCase('ru').includes(needle),
  );
  const current = Math.min(page, Math.max(0, Math.ceil(filtered.length / 20) - 1));
  return (
    <>
      <label>
        Поиск в выбранных
        <input
          type="search"
          value={query}
          onChange={(event) => {
            setQuery(event.target.value);
            setPage(0);
          }}
        />
      </label>
      <ul className="archive-selection" aria-label="Выбранные экземпляры">
        {filtered.slice(current * 20, (current + 1) * 20).map((item) => (
          <li key={item.id}>
            <strong>{item.name}</strong>
            <code>{item.id}</code>
            <span>{item.detail}</span>
          </li>
        ))}
      </ul>
      <nav className="instance-pager" aria-label="Страницы выбранных экземпляров">
        <button disabled={current === 0} onClick={() => setPage(current - 1)}>
          Назад
        </button>
        <span>
          {filtered.length ? current + 1 : 0} / {Math.ceil(filtered.length / 20)}
        </span>
        <button
          disabled={(current + 1) * 20 >= filtered.length}
          onClick={() => setPage(current + 1)}
        >
          Далее
        </button>
      </nav>
    </>
  );
}

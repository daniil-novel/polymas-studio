"""Build a single reading copy from the authoritative specification chapters."""

from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
CHAPTERS = [
    "specs/01-product-spec.md",
    "specs/02-ux-spec.md",
    "specs/03-architecture.md",
    "specs/04-integrations.md",
    "specs/05-delivery-and-acceptance.md",
    "design/design-spec.md",
    "specs/06-decisions-and-sources.md",
    "contracts/README.md",
]


def rebase_links(text: str, parent: Path) -> str:
    def replace(match: re.Match) -> str:
        target = match.group(2)
        if "://" in target or target.startswith("#"):
            return match.group(0)
        file, marker, anchor = target.partition("#")
        absolute = (parent / file).resolve()
        target = absolute.relative_to(ROOT).as_posix()
        if marker:
            target += "#" + anchor
        return match.group(1) + target + ")"
    return re.sub(r"(!?\[[^\]]*\]\()([^)]+)\)", replace, text)


def main() -> None:
    parts = [
        "# PolyMAS Studio — полная спецификация",
        "Версия 0.1.0 · 8 сентября 2026 · Проект спецификации для реализации.",
        "Сводный документ объединяет продукт, UX, архитектуру, интеграции, приёмку, дизайн и начальные контракты. Исходные редактируемые главы находятся в specs/, design/ и contracts/. Исполняющая платформа ещё не реализована.",
        "## Навигация",
    ]
    for index, chapter in enumerate(CHAPTERS, 1):
        title = (ROOT / chapter).read_text(encoding="utf-8").splitlines()[0].lstrip("# ")
        parts.append(f"{index}. [{title}](#chapter-{index})")
    parts.append("## Дизайн-референсы")
    for path, title in [
        ("01-agent-graph.png", "Граф агентов"),
        ("02-role-instances.png", "Роль и экземпляры"),
        ("03-mcp-setup.png", "Подключение MCP"),
    ]:
        parts.append(f"![{title}](design/references/{path})")
    parts.append("[Открыть интерактивный макет](design/prototype.html) · [Промпты генерации](design/prompts.md)")
    for index, chapter in enumerate(CHAPTERS, 1):
        path = ROOT / chapter
        parts.extend([f'<a id="chapter-{index}"></a>', "---", rebase_links(path.read_text(encoding="utf-8"), path.parent)])
    (ROOT / "SPECIFICATION.md").write_text("\n\n".join(parts) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()

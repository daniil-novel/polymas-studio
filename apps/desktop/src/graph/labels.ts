interface Rect {
  left: number;
  right: number;
  top: number;
  bottom: number;
}
interface Label {
  label: string | null;
  x: number;
  y: number;
  size: number;
}
const segmenter = new Intl.Segmenter(undefined, { granularity: 'grapheme' });

// A small spatial grid compares measured text rectangles, rather than node centers.
export class GraphLabels {
  private cells = new Map<string, Rect[]>();
  reset() {
    this.cells.clear();
  }
  draw(context: CanvasRenderingContext2D, node: Label, font: string) {
    if (!node.label) return;
    context.font = `400 12px ${font}`;
    let text = node.label;
    if (context.measureText(text).width > 220) {
      const characters = Array.from(segmenter.segment(text), (part) => part.segment);
      let low = 0,
        high = characters.length;
      while (low < high) {
        const middle = Math.ceil((low + high) / 2);
        if (context.measureText(characters.slice(0, middle).join('') + '…').width <= 220)
          low = middle;
        else high = middle - 1;
      }
      text = characters.slice(0, low).join('') + '…';
    }
    const x = node.x + node.size + 4;
    const rect = {
      left: x - 3,
      right: x + context.measureText(text).width + 3,
      top: node.y - 9,
      bottom: node.y + 9,
    };
    const keys = [];
    for (let col = Math.floor(rect.left / 32); col <= Math.floor(rect.right / 32); col++) {
      for (let row = Math.floor(rect.top / 32); row <= Math.floor(rect.bottom / 32); row++) {
        const key = `${col}:${row}`;
        if (
          this.cells
            .get(key)
            ?.some(
              (other) =>
                rect.left < other.right &&
                rect.right > other.left &&
                rect.top < other.bottom &&
                rect.bottom > other.top,
            )
        )
          return;
        keys.push(key);
      }
    }
    for (const key of keys) {
      const list = this.cells.get(key) ?? [];
      list.push(rect);
      this.cells.set(key, list);
    }
    context.fillStyle = '#E7EDF5';
    context.textAlign = 'left';
    context.textBaseline = 'middle';
    context.fillText(text, x, node.y);
  }
}

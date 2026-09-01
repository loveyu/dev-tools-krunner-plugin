export type PickedColor = {
  readonly hex: string;
  readonly red: number;
  readonly green: number;
  readonly blue: number;
};

export type ColorFormats = PickedColor & {
  readonly rgb: string;
  readonly hsl: string;
};

export function normalizeHex(value: string): string | null {
  const compact = value.trim().replace(/^#/u, '');
  const expanded =
    compact.length === 3
      ? compact
          .split('')
          .map((channel) => channel + channel)
          .join('')
      : compact;
  return /^[\dA-Fa-f]{6}$/u.test(expanded) ? `#${expanded.toUpperCase()}` : null;
}

export function colorFromHex(value: string): ColorFormats | null {
  const hex = normalizeHex(value);
  if (hex === null) return null;
  return colorFromRgb(
    Number.parseInt(hex.slice(1, 3), 16),
    Number.parseInt(hex.slice(3, 5), 16),
    Number.parseInt(hex.slice(5, 7), 16),
  );
}

export function colorFromRgb(red: number, green: number, blue: number): ColorFormats {
  const channels = [red, green, blue].map((channel) =>
    Math.round(Math.min(255, Math.max(0, channel))),
  );
  const [r = 0, g = 0, b = 0] = channels;
  const hex = `#${[r, g, b]
    .map((channel) => channel.toString(16).padStart(2, '0'))
    .join('')
    .toUpperCase()}`;
  const [hue, saturation, lightness] = rgbToHsl(r, g, b);
  return {
    hex,
    red: r,
    green: g,
    blue: b,
    rgb: `rgb(${String(r)}, ${String(g)}, ${String(b)})`,
    hsl: `hsl(${String(hue)}, ${String(saturation)}%, ${String(lightness)}%)`,
  };
}

export function colorFromHsv(hue: number, saturation: number, value: number): ColorFormats {
  const normalizedHue = ((hue % 360) + 360) % 360;
  const normalizedSaturation = Math.min(1, Math.max(0, saturation));
  const normalizedValue = Math.min(1, Math.max(0, value));
  const chroma = normalizedValue * normalizedSaturation;
  const sector = normalizedHue / 60;
  const intermediate = chroma * (1 - Math.abs((sector % 2) - 1));
  const offset = normalizedValue - chroma;
  const [red, green, blue] =
    sector < 1
      ? [chroma, intermediate, 0]
      : sector < 2
        ? [intermediate, chroma, 0]
        : sector < 3
          ? [0, chroma, intermediate]
          : sector < 4
            ? [0, intermediate, chroma]
            : sector < 5
              ? [intermediate, 0, chroma]
              : [chroma, 0, intermediate];
  return colorFromRgb((red + offset) * 255, (green + offset) * 255, (blue + offset) * 255);
}

function rgbToHsl(red: number, green: number, blue: number): readonly [number, number, number] {
  const r = red / 255;
  const g = green / 255;
  const b = blue / 255;
  const maximum = Math.max(r, g, b);
  const minimum = Math.min(r, g, b);
  const delta = maximum - minimum;
  const lightness = (maximum + minimum) / 2;
  if (delta === 0) return [0, 0, Math.round(lightness * 100)];
  const saturation = delta / (1 - Math.abs(2 * lightness - 1));
  const rawHue =
    maximum === r
      ? ((g - b) / delta) % 6
      : maximum === g
        ? (b - r) / delta + 2
        : (r - g) / delta + 4;
  const hue = Math.round((rawHue * 60 + 360) % 360);
  return [hue, Math.round(saturation * 100), Math.round(lightness * 100)];
}

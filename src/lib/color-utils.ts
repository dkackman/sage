interface ParsedColor {
  format: 'hsl' | 'rgb' | 'hex' | 'named' | 'other';
  value: string;
}

export function parseColor(color: string): ParsedColor {
  if (!color) {
    return { format: 'other', value: '' };
  }

  // Check for color functions (hsl, rgb, etc.)
  if (color.includes('(') && color.includes(')')) {
    if (color.startsWith('hsl')) {
      return { format: 'hsl', value: color };
    } else if (color.startsWith('rgb')) {
      return { format: 'rgb', value: color };
    }
    return { format: 'other', value: color };
  }

  // Check for hex colors
  if (color.startsWith('#')) {
    return { format: 'hex', value: color };
  }

  // Check for named colors
  if (/^[a-zA-Z]+$/.test(color)) {
    return { format: 'named', value: color };
  }

  return { format: 'other', value: color };
}

/**
 * Creates a semi-transparent version of a color.
 * Works with any color format.
 */
export function makeColorTransparent(color: string, opacity: number): string {
  const parsed = parseColor(color);

  // For HSL, extract and rebuild with alpha
  if (parsed.format === 'hsl') {
    const match = parsed.value.match(/hsl\((.*?)\)/);
    if (match) {
      return `hsla(${match[1]} / ${opacity})`;
    }
  }

  // For hex colors, convert to RGBA
  if (parsed.format === 'hex') {
    const hex = parsed.value.replace('#', '');
    const r = parseInt(hex.substr(0, 2), 16);
    const g = parseInt(hex.substr(2, 2), 16);
    const b = parseInt(hex.substr(4, 2), 16);
    return `rgba(${r}, ${g}, ${b}, ${opacity})`;
  }

  // For RGB colors, convert to RGBA
  if (parsed.format === 'rgb') {
    const match = parsed.value.match(/rgb\((.*?)\)/);
    if (match) {
      return `rgba(${match[1]}, ${opacity})`;
    }
  }

  // For named colors or other formats, use CSS color-mix (modern browsers)
  // Fallback to the original color if color-mix is not supported
  return `color-mix(in srgb, ${parsed.value} ${opacity * 100}%, transparent)`;
}

export function getColorLightness(color: string): number {
  const parsed = parseColor(color);

  // For HSL values, we can extract lightness directly
  if (parsed.format === 'hsl') {
    const lightnessMatch = parsed.value.match(/(\d+(?:\.\d+)?)%\s*(?:\)|$)/);
    if (lightnessMatch) {
      return parseFloat(lightnessMatch[1]);
    }
  }

  // For other formats, we need to convert to RGB first
  // This requires DOM API, so we'll use a temporary element
  if (typeof document !== 'undefined') {
    try {
      const tempElement = document.createElement('div');
      tempElement.style.color = color;
      tempElement.style.display = 'none';
      document.body.appendChild(tempElement);

      const computedColor = getComputedStyle(tempElement).color;
      document.body.removeChild(tempElement);

      // Parse RGB values
      const rgbMatch = computedColor.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)/);
      if (rgbMatch) {
        const r = parseInt(rgbMatch[1]) / 255;
        const g = parseInt(rgbMatch[2]) / 255;
        const b = parseInt(rgbMatch[3]) / 255;

        // Calculate lightness using HSL formula
        const max = Math.max(r, g, b);
        const min = Math.min(r, g, b);
        const lightness = ((max + min) / 2) * 100;

        return Math.round(lightness);
      }
    } catch (error) {
      console.warn('Could not determine color lightness:', color, error);
    }
  }

  return 50;
}

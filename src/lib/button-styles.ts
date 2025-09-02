import { Theme } from './theme.type';

// Base button style effects that can be applied to any variant
export interface ButtonStyleEffect {
    name: string;
    // CSS generator function that takes variant name and returns CSS rules
    generateCSS: (variant: string) => string;
    // Optional: CSS variables this effect depends on
    //requiredVariables?: string[];
}

// Built-in button style effects
export const builtInButtonStyles: Record<string, ButtonStyleEffect> = {
    gradient: {
        name: 'gradient',
        generateCSS: (variant: string) => `
      .btn-variant-${variant}[data-theme-style*='gradient'] {
        background: linear-gradient(135deg, var(--btn-${variant}-bg), var(--btn-${variant}-hover-bg));
        color: var(--btn-${variant}-color);
        border: var(--btn-${variant}-border);
      }
    `,
    },
    shimmer: {
        name: 'shimmer',
        generateCSS: (variant: string) => `
      .btn-variant-${variant}[data-theme-style*='shimmer'] {
        background: var(--btn-${variant}-bg);
        border: var(--btn-${variant}-border);
        color: var(--btn-${variant}-color);
        position: relative;
        overflow: hidden;
        opacity: 0.8;
      }
      
      .btn-variant-${variant}[data-theme-style*='shimmer']::before {
        content: '';
        position: absolute;
        top: 0;
        left: -100%;
        width: 100%;
        height: 100%;
        background: linear-gradient(90deg, transparent, var(--btn-${variant}-hover-color), transparent);
        opacity: 0.2;
        transition: left 0.3s ease;
      }
      
      .btn-variant-${variant}[data-theme-style*='shimmer']:hover::before {
        left: 100%;
      }
      
      .btn-variant-${variant}[data-theme-style*='shimmer']:hover {
        ${variant === 'outline' ? 'border-color: var(--btn-outline-hover-border-color);' : ''}
        ${variant === 'outline' ? 'color: var(--btn-outline-hover-color);' : ''}
        opacity: 1;
      }
    `,
    },

    'pixel-art': {
        name: 'pixel-art',
        generateCSS: (variant: string) => `
      .btn-variant-${variant}[data-theme-style*='pixel-art'] {
        background: var(--btn-${variant}-bg);
        color: var(--btn-${variant}-color);
        border: var(--btn-${variant}-border);
        box-shadow: var(--btn-${variant}-shadow);
        image-rendering: pixelated;
        image-rendering: -moz-crisp-edges;
        image-rendering: crisp-edges;
      }
      
      .btn-variant-${variant}[data-theme-style*='pixel-art']:hover {
        transform: var(--btn-${variant}-hover-transform);
        box-shadow: var(--btn-${variant}-hover-shadow);
      }
    `,
    },

    '3d-effects': {
        name: '3d-effects',
        generateCSS: (variant: string) => `
      .btn-variant-${variant}[data-theme-style*='3d-effects'] {
        background: var(--btn-${variant}-bg);
        color: var(--btn-${variant}-color);
        border: var(--btn-${variant}-border);
        box-shadow: var(--btn-${variant}-shadow);
      }
      
      .btn-variant-${variant}[data-theme-style*='3d-effects']:active {
        border-style: var(--btn-${variant}-active-border-style);
        box-shadow: var(--btn-${variant}-active-shadow);
      }
    `,
    },

    'rounded-buttons': {
        name: 'rounded-buttons',
        generateCSS: (variant: string) => `
      .btn-variant-${variant}[data-theme-style*='rounded-buttons'] {
        background: var(--btn-${variant}-bg);
        color: var(--btn-${variant}-color);
        border: var(--btn-${variant}-border);
        border-radius: var(--btn-${variant}-radius);
        box-shadow: var(--btn-${variant}-shadow);
      }
    `,
    },
};

// Button variants that styles should be applied to
export const buttonVariants = ['default', 'outline', 'secondary', 'destructive', 'ghost', 'link'];

// Generate CSS for all button style combinations
export function generateButtonStyleCSS(theme: Theme): string {
    const buttonStyles = theme.buttonStyles || [];
    //const customStyles = theme.customButtonStyles || {};

    let css = '';

    // Generate CSS for built-in styles
    buttonStyles.forEach(styleName => {
        const styleEffect = builtInButtonStyles[styleName];
        if (styleEffect) {
            buttonVariants.forEach(variant => {
                css += styleEffect.generateCSS(variant);
            });
        }
    });

    // Generate CSS for custom theme-specific styles
    // Object.entries(customStyles).forEach(([styleName, styleEffect]) => {
    //     if (buttonStyles.includes(styleName)) {
    //         buttonVariants.forEach(variant => {
    //             css += styleEffect.generateCSS(variant);
    //         });
    //     }
    // });

    return css;
}

// Apply dynamic button styles to the document
export function applyDynamicButtonStyles(theme: Theme): void {
    const existingStyleElement = document.getElementById('dynamic-button-styles');
    if (existingStyleElement) {
        existingStyleElement.remove();
    }

    const css = generateButtonStyleCSS(theme);
    if (css) {
        const styleElement = document.createElement('style');
        styleElement.id = 'dynamic-button-styles';
        styleElement.textContent = css;
        document.head.appendChild(styleElement);
    }
}

// Helper function to register a new button style effect
export function registerButtonStyle(name: string, effect: ButtonStyleEffect): void {
    builtInButtonStyles[name] = effect;
}

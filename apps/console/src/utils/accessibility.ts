export function generateId(prefix: string): string {
  return `${prefix}-${Math.random().toString(36).slice(2, 9)}`;
}

export function useId(prefix: string): string {
  const { useRef } = require('react');
  const idRef = useRef<string>();
  if (!idRef.current) {
    idRef.current = generateId(prefix);
  }
  return idRef.current;
}

export function announceToScreenReader(message: string, priority: 'polite' | 'assertive' = 'polite') {
  const announcer = document.getElementById('screen-reader-announcer');
  if (announcer) {
    announcer.setAttribute('aria-live', priority);
    announcer.textContent = message;
  }
}

export function trapFocus(element: HTMLElement) {
  const focusableElements = element.querySelectorAll<HTMLElement>(
    'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
  );
  const firstElement = focusableElements[0];
  const lastElement = focusableElements[focusableElements.length - 1];

  function handleTab(e: KeyboardEvent) {
    if (e.key !== 'Tab') return;

    if (e.shiftKey) {
      if (document.activeElement === firstElement) {
        e.preventDefault();
        lastElement?.focus();
      }
    } else {
      if (document.activeElement === lastElement) {
        e.preventDefault();
        firstElement?.focus();
      }
    }
  }

  element.addEventListener('keydown', handleTab);
  firstElement?.focus();

  return () => {
    element.removeEventListener('keydown', handleTab);
  };
}

export function focusFirstError(form: HTMLFormElement) {
  const firstError = form.querySelector('[aria-invalid="true"]') as HTMLElement;
  firstError?.focus();
}

export function getFocusableChildren(container: HTMLElement): HTMLElement[] {
  return Array.from(
    container.querySelectorAll<HTMLElement>(
      'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
    )
  ).filter((el) => !el.hasAttribute('disabled') && el.offsetParent !== null);
}

export function moveFocus(direction: 'next' | 'prev', container: HTMLElement) {
  const focusable = getFocusableChildren(container);
  const currentIndex = focusable.indexOf(document.activeElement as HTMLElement);
  if (currentIndex === -1) return;

  const nextIndex = direction === 'next' ? currentIndex + 1 : currentIndex - 1;
  if (nextIndex >= 0 && nextIndex < focusable.length) {
    focusable[nextIndex].focus();
  }
}

export function skipLink(id: string) {
  const element = document.getElementById(id);
  element?.focus();
  element?.scrollIntoView({ behavior: 'smooth', block: 'center' });
}

export const KEYS = {
  ENTER: 'Enter',
  SPACE: ' ',
  ESCAPE: 'Escape',
  TAB: 'Tab',
  ARROW_UP: 'ArrowUp',
  ARROW_DOWN: 'ArrowDown',
  ARROW_LEFT: 'ArrowLeft',
  ARROW_RIGHT: 'ArrowRight',
  HOME: 'Home',
  END: 'End',
} as const;

export function isActivationKey(key: string): boolean {
  return key === KEYS.ENTER || key === KEYS.SPACE;
}

export function useKeyboardNavigation(
  items: HTMLElement[],
  onSelect: (index: number) => void,
  options: { loop?: boolean; orientation?: 'horizontal' | 'vertical' } = {}
) {
  const { useCallback, useRef } = require('react');
  const { loop = true, orientation = 'vertical' } = options;
  const focusedIndexRef = useRef(-1);

  return useCallback((e: React.KeyboardEvent) => {
    const { key } = e;
    const length = items.length;
    if (length === 0) return;

    let newIndex = focusedIndexRef.current;

    const isHorizontal = orientation === 'horizontal';
    const nextKey = isHorizontal ? KEYS.ARROW_RIGHT : KEYS.ARROW_DOWN;
    const prevKey = isHorizontal ? KEYS.ARROW_LEFT : KEYS.ARROW_UP;

    switch (key) {
      case nextKey:
        e.preventDefault();
        newIndex = focusedIndexRef.current + 1;
        if (newIndex >= length) {
          newIndex = loop ? 0 : length - 1;
        }
        break;
      case prevKey:
        e.preventDefault();
        newIndex = focusedIndexRef.current - 1;
        if (newIndex < 0) {
          newIndex = loop ? length - 1 : 0;
        }
        break;
      case KEYS.HOME:
        e.preventDefault();
        newIndex = 0;
        break;
      case KEYS.END:
        e.preventDefault();
        newIndex = length - 1;
        break;
      case KEYS.ENTER:
      case KEYS.SPACE:
        e.preventDefault();
        onSelect(focusedIndexRef.current);
        return;
      default:
        return;
    }

    focusedIndexRef.current = newIndex;
    items[newIndex]?.focus();
    onSelect(newIndex);
  }, [items, onSelect, loop, orientation]);
}

export function createScreenReaderAnnouncer() {
  const announcer = document.createElement('div');
  announcer.id = 'screen-reader-announcer';
  announcer.setAttribute('aria-live', 'polite');
  announcer.setAttribute('aria-atomic', 'true');
  announcer.style.position = 'absolute';
  announcer.style.left = '-9999px';
  announcer.style.width = '1px';
  announcer.style.height = '1px';
  announcer.style.overflow = 'hidden';
  document.body.appendChild(announcer);
  return announcer;
}

export function removeScreenReaderAnnouncer() {
  const announcer = document.getElementById('screen-reader-announcer');
  announcer?.remove();
}

export function checkColorContrast(foreground: string, background: string): number {
  const getLuminance = (color: string) => {
    const rgb = hexToRgb(color);
    if (!rgb) return 0;
    const [r, g, b] = [rgb.r, rgb.g, rgb.b].map((v) => {
      v /= 255;
      return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
    });
    return 0.2126 * r + 0.7152 * g + 0.0722 * b;
  };

  const hexToRgb = (hex: string) => {
    const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex);
    return result
      ? {
          r: parseInt(result[1], 16),
          g: parseInt(result[2], 16),
          b: parseInt(result[3], 16),
        }
      : null;
  };

  const l1 = getLuminance(foreground);
  const l2 = getLuminance(background);
  const lighter = Math.max(l1, l2);
  const darker = Math.min(l1, l2);
  return (lighter + 0.05) / (darker + 0.05);
}

export function meetsWCAGAA(foreground: string, background: string, largeText = false): boolean {
  const ratio = checkColorContrast(foreground, background);
  return largeText ? ratio >= 3 : ratio >= 4.5;
}

export function meetsWCAGAAA(foreground: string, background: string, largeText = false): boolean {
  const ratio = checkColorContrast(foreground, background);
  return largeText ? ratio >= 4.5 : ratio >= 7;
}
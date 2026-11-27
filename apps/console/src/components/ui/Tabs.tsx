import { forwardRef, useState, useRef, useEffect, createContext, useContext } from 'react';
import { clsx } from 'clsx';

interface TabsContextValue {
  value: string;
  onValueChange: (value: string) => void;
  orientation: 'horizontal' | 'vertical';
  activationMode: 'automatic' | 'manual';
}

const TabsContext = createContext<TabsContextValue | null>(null);

function useTabsContext() {
  const context = useContext(TabsContext);
  if (!context) {
    throw new Error('Tabs components must be used within Tabs');
  }
  return context;
}

interface TabsProps {
  defaultValue: string;
  value?: string;
  onValueChange?: (value: string) => void;
  children: React.ReactNode;
  className?: string;
  orientation?: 'horizontal' | 'vertical';
  activationMode?: 'automatic' | 'manual';
}

export const Tabs = forwardRef<HTMLDivElement, TabsProps>(
  ({ defaultValue, value, onValueChange, children, className, orientation = 'horizontal', activationMode = 'automatic' }, ref) => {
    const [internalValue, setInternalValue] = useState(defaultValue);
    const controlled = value !== undefined;
    const currentValue = controlled ? value : internalValue;

    const handleValueChange = (newValue: string) => {
      if (!controlled) {
        setInternalValue(newValue);
      }
      onValueChange?.(newValue);
    };

    const contextValue: TabsContextValue = {
      value: currentValue,
      onValueChange: handleValueChange,
      orientation,
      activationMode,
    };

    return (
      <TabsContext.Provider value={contextValue}>
        <div ref={ref} className={clsx('w-full', className)} data-orientation={orientation}>
          {children}
        </div>
      </TabsContext.Provider>
    );
  }
);
Tabs.displayName = 'Tabs';

interface TabsListProps {
  children: React.ReactNode;
  className?: string;
  'aria-label'?: string;
  'aria-labelledby'?: string;
}

export const TabsList = forwardRef<HTMLDivElement, TabsListProps>(
  ({ children, className, 'aria-label': ariaLabel, 'aria-labelledby': ariaLabelledby }, ref) => {
    const { orientation, activationMode } = useTabsContext();

    return (
      <div
        ref={ref}
        role="tablist"
        aria-label={ariaLabel}
        aria-labelledby={ariaLabelledby}
        aria-orientation={orientation}
        data-activation-mode={activationMode}
        className={clsx(
          'inline-flex items-center justify-center rounded-md bg-secondary-100 dark:bg-secondary-800 p-1',
          orientation === 'vertical' && 'flex-col',
          className
        )}
      >
        {children}
      </div>
    );
  }
);
TabsList.displayName = 'TabsList';

interface TabsTriggerProps {
  value: string;
  children: React.ReactNode;
  disabled?: boolean;
  className?: string;
}

export const TabsTrigger = forwardRef<HTMLButtonElement, TabsTriggerProps>(
  ({ value, children, disabled, className }, ref) => {
    const { value: currentValue, onValueChange, orientation, activationMode } = useTabsContext();
    const isActive = currentValue === value;
    const triggerRef = useRef<HTMLButtonElement>(null);

    const handleClick = () => {
      if (!disabled) {
        onValueChange(value);
      }
    };

    const handleKeyDown = (e: React.KeyboardEvent) => {
      if (disabled) return;

      const tabsList = triggerRef.current?.closest('[role="tablist"]');
      if (!tabsList) return;

      const triggers = Array.from(
        tabsList.querySelectorAll<HTMLButtonElement>('[role="tab"]:not(:disabled)')
      );
      const currentIndex = triggers.indexOf(triggerRef.current!);

      let newIndex = currentIndex;

      const isHorizontal = orientation === 'horizontal';
      const nextKey = isHorizontal ? 'ArrowRight' : 'ArrowDown';
      const prevKey = isHorizontal ? 'ArrowLeft' : 'ArrowUp';

      switch (e.key) {
        case nextKey:
          e.preventDefault();
          newIndex = (currentIndex + 1) % triggers.length;
          break;
        case prevKey:
          e.preventDefault();
          newIndex = (currentIndex - 1 + triggers.length) % triggers.length;
          break;
        case 'Home':
          e.preventDefault();
          newIndex = 0;
          break;
        case 'End':
          e.preventDefault();
          newIndex = triggers.length - 1;
          break;
        default:
          return;
      }

      triggers[newIndex]?.focus();
      if (activationMode === 'automatic') {
        onValueChange(triggers[newIndex].dataset.value!);
      }
    };

    return (
      <button
        ref={(el) => {
          triggerRef.current = el;
          if (ref && typeof ref === 'function') ref(el);
          else if (ref && typeof ref === 'object') ref.current = el;
        }}
        role="tab"
        aria-selected={isActive}
        aria-disabled={disabled}
        aria-controls={`panel-${value}`}
        id={`tab-${value}`}
        data-value={value}
        data-state={isActive ? 'active' : 'inactive'}
        disabled={disabled}
        tabIndex={isActive ? 0 : -1}
        onClick={handleClick}
        onKeyDown={handleKeyDown}
        className={clsx(
          'inline-flex items-center justify-center whitespace-nowrap rounded-sm px-3 py-1.5 text-sm font-medium transition-all focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-500 focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50',
          isActive
            ? 'bg-white dark:bg-secondary-700 text-secondary-900 dark:text-white shadow-sm'
            : 'text-secondary-600 dark:text-secondary-400 hover:text-secondary-900 dark:hover:text-white',
          className
        )}
      >
        {children}
      </button>
    );
  }
);
TabsTrigger.displayName = 'TabsTrigger';

interface TabsContentProps {
  value: string;
  children: React.ReactNode;
  className?: string;
}

export const TabsContent = forwardRef<HTMLDivElement, TabsContentProps>(
  ({ value, children, className }, ref) => {
    const { value: currentValue } = useTabsContext();
    const isActive = currentValue === value;

    return (
      <div
        ref={ref}
        role="tabpanel"
        id={`panel-${value}`}
        aria-labelledby={`tab-${value}`}
        data-value={value}
        data-state={isActive ? 'active' : 'inactive'}
        className={clsx(
          'mt-2 ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2',
          className
        )}
        hidden={!isActive}
        tabIndex={0}
      >
        {isActive ? children : null}
      </div>
    );
  }
);
TabsContent.displayName = 'TabsContent';
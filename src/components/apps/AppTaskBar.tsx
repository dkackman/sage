import { Button } from '@/components/ui/button.tsx';
import { LayoutGrid, X } from 'lucide-react';
import clsx from 'clsx';
import { useEffect, useMemo, useRef, useState } from 'react';

export interface AppTaskBarTab {
  appId: string;
  name: string;
  iconSrc: string | null;
  isActive: boolean;
}

interface Props {
  tabs: AppTaskBarTab[];
  onOpenApps: () => void;
  onSelectApp: (appId: string) => void;
  onCloseApp: (appId: string) => void;
  onReorderTabs: (nextAppIds: string[]) => void;
}

interface DragState {
  draggedAppId: string;
  pointerOffsetWithinTab: number;
  currentPointerX: number;
  overlayLeftPx: number;
}

function reorderIds(
  ids: string[],
  fromIndex: number,
  toIndex: number,
): string[] {
  if (fromIndex === toIndex) {
    return ids;
  }

  const next = [...ids];
  const [moved] = next.splice(fromIndex, 1);
  next.splice(toIndex, 0, moved);
  return next;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

export function AppTaskBar({
  tabs,
  onOpenApps,
  onSelectApp,
  onCloseApp,
  onReorderTabs,
}: Props) {
  const tabsContainerRef = useRef<HTMLDivElement | null>(null);

  const [dragState, setDragState] = useState<DragState | null>(null);
  const [previewOrder, setPreviewOrder] = useState<string[] | null>(null);

  const baseOrder = useMemo(() => tabs.map((tab) => tab.appId), [tabs]);
  const activeOrder = previewOrder ?? baseOrder;

  const tabsById = useMemo(() => {
    return new Map(tabs.map((tab) => [tab.appId, tab]));
  }, [tabs]);

  const orderedTabs = useMemo(() => {
    return activeOrder
      .map((appId) => tabsById.get(appId))
      .filter((tab): tab is AppTaskBarTab => tab != null);
  }, [activeOrder, tabsById]);

  const draggedIndex = dragState
    ? activeOrder.indexOf(dragState.draggedAppId)
    : -1;

  useEffect(() => {
    if (!dragState) {
      setPreviewOrder(null);
    }
  }, [dragState]);

  useEffect(() => {
    if (!dragState) {
      return;
    }

    const handlePointerMove = (event: PointerEvent) => {
      setDragState((prev) =>
        prev
          ? {
              ...prev,
              currentPointerX: event.clientX,
            }
          : null,
      );
    };

    const handlePointerUp = () => {
      setDragState((prev) => {
        if (!prev) {
          return null;
        }

        if (previewOrder) {
          onReorderTabs(previewOrder);
        }

        return null;
      });
    };

    window.addEventListener('pointermove', handlePointerMove);
    window.addEventListener('pointerup', handlePointerUp);

    return () => {
      window.removeEventListener('pointermove', handlePointerMove);
      window.removeEventListener('pointerup', handlePointerUp);
    };
  }, [dragState, previewOrder, onReorderTabs]);

  useEffect(() => {
    if (!dragState) {
      return;
    }

    const containerEl = tabsContainerRef.current;
    if (!containerEl) {
      return;
    }

    const tabCount = activeOrder.length;
    if (tabCount === 0) {
      return;
    }

    const containerRect = containerEl.getBoundingClientRect();
    const slotWidth = containerRect.width / tabCount;

    const minOverlayLeft = 0;
    const maxOverlayLeft = Math.max(0, containerRect.width - slotWidth);

    const rawOverlayLeft =
      dragState.currentPointerX -
      containerRect.left -
      dragState.pointerOffsetWithinTab;

    const clampedOverlayLeft = clamp(
      rawOverlayLeft,
      minOverlayLeft,
      maxOverlayLeft,
    );

    const draggedCenterX = clampedOverlayLeft + slotWidth / 2;
    const nextIndex = clamp(
      Math.floor(draggedCenterX / slotWidth),
      0,
      tabCount - 1,
    );

    setDragState((prev) =>
      prev
        ? {
            ...prev,
            overlayLeftPx: clampedOverlayLeft,
          }
        : null,
    );

    const currentIndex = activeOrder.indexOf(dragState.draggedAppId);
    if (currentIndex === -1 || nextIndex === currentIndex) {
      return;
    }

    setPreviewOrder(reorderIds(activeOrder, currentIndex, nextIndex));
  }, [dragState, activeOrder]);

  return (
    <div className='flex h-12 shrink-0 items-end gap-2 border-b bg-muted/30 px-3 pt-2'>
      <Button
        variant='ghost'
        className='h-9 shrink-0 px-3'
        onClick={onOpenApps}
      >
        <LayoutGrid className='mr-2 h-4 w-4' />
        Apps
      </Button>

      <div
        ref={tabsContainerRef}
        className='relative flex min-w-0 flex-1 items-end gap-1 overflow-hidden'
      >
        {orderedTabs.map((tab) => {
          const isDragged = dragState?.draggedAppId === tab.appId;

          return (
            <div
              key={tab.appId}
              className={clsx('min-w-0 flex-1', isDragged && 'opacity-0')}
            >
              <button
                type='button'
                onClick={() => {
                  if (!dragState) {
                    onSelectApp(tab.appId);
                  }
                }}
                onPointerDown={(event) => {
                  if (event.button !== 0) {
                    return;
                  }

                  const containerEl = tabsContainerRef.current;
                  if (!containerEl) {
                    return;
                  }

                  const containerRect = containerEl.getBoundingClientRect();
                  const tabCount = baseOrder.length;
                  if (tabCount === 0) {
                    return;
                  }

                  const slotWidth = containerRect.width / tabCount;
                  const currentIndex = activeOrder.indexOf(tab.appId);
                  const slotLeft = currentIndex * slotWidth;
                  const pointerXWithinContainer =
                    event.clientX - containerRect.left;
                  const pointerOffsetWithinTab = clamp(
                    pointerXWithinContainer - slotLeft,
                    0,
                    slotWidth,
                  );

                  setPreviewOrder((prev) => prev ?? baseOrder);
                  setDragState({
                    draggedAppId: tab.appId,
                    pointerOffsetWithinTab,
                    currentPointerX: event.clientX,
                    overlayLeftPx: slotLeft,
                  });
                }}
                className={clsx(
                  'group flex h-9 w-full items-center gap-2 rounded-t-md border border-b-0 px-3 text-left transition-[background-color,color] duration-150 select-none',
                  tab.isActive
                    ? 'bg-background'
                    : 'bg-muted text-muted-foreground hover:bg-muted/80',
                )}
              >
                {tab.iconSrc ? (
                  <img
                    src={tab.iconSrc}
                    alt=''
                    className='h-4 w-4 shrink-0 rounded-sm'
                  />
                ) : (
                  <div className='flex h-4 w-4 shrink-0 items-center justify-center rounded-sm bg-border text-[10px] font-semibold'>
                    {tab.name.slice(0, 1).toUpperCase()}
                  </div>
                )}

                <span className='min-w-0 flex-1 truncate text-sm font-medium'>
                  {tab.name}
                </span>

                <span
                  className={clsx(
                    'shrink-0',
                    tab.isActive
                      ? 'opacity-100'
                      : 'opacity-0 transition-opacity group-hover:opacity-100',
                  )}
                >
                  <Button
                    type='button'
                    variant='ghost'
                    size='icon'
                    className='h-6 w-6'
                    onClick={(event) => {
                      event.stopPropagation();
                      onCloseApp(tab.appId);
                    }}
                  >
                    <X className='h-3.5 w-3.5' />
                  </Button>
                </span>
              </button>
            </div>
          );
        })}

        {dragState && draggedIndex !== -1
          ? (() => {
              const draggedTab = tabsById.get(dragState.draggedAppId);
              if (!draggedTab) {
                return null;
              }

              return (
                <div
                  className='pointer-events-none absolute bottom-0 z-20'
                  style={{
                    left: `${dragState.overlayLeftPx}px`,
                    width: `${100 / activeOrder.length}%`,
                  }}
                >
                  <div
                    className={clsx(
                      'flex h-9 w-full items-center gap-2 rounded-t-md border border-b-0 px-3 text-left shadow-sm',
                      draggedTab.isActive
                        ? 'bg-background'
                        : 'bg-muted text-muted-foreground',
                    )}
                  >
                    {draggedTab.iconSrc ? (
                      <img
                        src={draggedTab.iconSrc}
                        alt=''
                        className='h-4 w-4 shrink-0 rounded-sm'
                      />
                    ) : (
                      <div className='flex h-4 w-4 shrink-0 items-center justify-center rounded-sm bg-border text-[10px] font-semibold'>
                        {draggedTab.name.slice(0, 1).toUpperCase()}
                      </div>
                    )}

                    <span className='min-w-0 flex-1 truncate text-sm font-medium'>
                      {draggedTab.name}
                    </span>

                    <span className='shrink-0 opacity-100'>
                      <div className='h-6 w-6' />
                    </span>
                  </div>
                </div>
              );
            })()
          : null}
      </div>
    </div>
  );
}

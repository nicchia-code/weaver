import { Component, useLayoutEffect, useRef, type ReactNode } from "react";

/** Every change of the Board that moves: a card or ghost slides, enters or leaves in 150 ms, ease-out. */
export const MOTION_MS = 150;
export const MOTION_EASING = "ease-out";

/** The system asks for reduced motion: every change is instant. */
export function reducedMotion(): boolean {
  return typeof window !== "undefined" && typeof window.matchMedia === "function" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

/** Plays `keyframes` on `element` for 150 ms, ease-out; nothing under reduced motion or without the Web Animations API. */
function play(element: Element, keyframes: Keyframe[], options: KeyframeAnimationOptions = {}): void {
  if (reducedMotion() || typeof element.animate !== "function") return;
  element.animate(keyframes, { duration: MOTION_MS, easing: MOTION_EASING, ...options });
}

/** An element that shows when its column becomes empty fades in; one that is there from the start does not. */
export function FadeIn({ animate, children }: { animate: boolean; children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    if (animate && ref.current) play(ref.current, [{ opacity: 0 }, { opacity: 1 }]);
    // Only at mount: the element is there from then on.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  return <div ref={ref}>{children}</div>;
}

/** One element of a [`MotionList`]: its key, which stays the same across pictures, and what it shows. */
export interface MotionItem {
  key: string;
  node: ReactNode;
}

interface Entry extends MotionItem {
  /** It is no longer in the list and only stays for its fade. */
  leaving: boolean;
}

interface Props {
  items: readonly MotionItem[];
}

interface State {
  source: readonly MotionItem[];
  entries: Entry[];
}

interface Place {
  top: number;
  left: number;
  width: number;
}

interface Before {
  places: Map<string, Place>;
  /** The element of the list that had the focus, when one did. */
  focused: HTMLElement | null;
}

function placeOf(element: HTMLElement): Place {
  return { top: element.offsetTop, left: element.offsetLeft, width: element.offsetWidth };
}

/**
 * A column's cards and ghosts, in the order they are given. What stays in the list and changes
 * place slides to its new place; what enters fades in; what leaves fades out, in the same picture
 * that causes it, and is `inert` and hidden from screen readers from the start of its fade. The
 * leaving element is taken out of the flow where it was, so the others move at once. A moved
 * element keeps the focus. Under reduced motion every change is instant. The list never sorts.
 */
export class MotionList extends Component<Props, State> {
  state: State = { source: this.props.items, entries: this.props.items.map((item) => ({ ...item, leaving: false })) };

  private container: HTMLElement | null = null;
  private readonly elements = new Map<string, HTMLElement>();
  private readonly timers = new Map<string, number>();

  static getDerivedStateFromProps(props: Props, state: State): Partial<State> | null {
    if (props.items === state.source) return null;
    const entries: Entry[] = props.items.map((item) => ({ ...item, leaving: false }));
    if (!reducedMotion()) {
      // What is no longer in the list stays where it was in the order, to fade out.
      const present = new Set(entries.map((entry) => entry.key));
      state.entries.forEach((old, index) => {
        if (present.has(old.key)) return;
        let at = 0;
        for (let before = index - 1; before >= 0; before--) {
          const found = entries.findIndex((entry) => entry.key === state.entries[before]?.key);
          if (found >= 0) {
            at = found + 1;
            break;
          }
        }
        entries.splice(at, 0, { key: old.key, node: old.node, leaving: true });
        present.add(old.key);
      });
    }
    return { source: props.items, entries };
  }

  getSnapshotBeforeUpdate(_props: Props, prevState: State): Before | null {
    if (prevState.entries === this.state.entries) return null;
    const places = new Map<string, Place>();
    for (const [key, element] of this.elements) places.set(key, placeOf(element));
    const active = document.activeElement;
    const focused = active instanceof HTMLElement && this.container?.contains(active) ? active : null;
    return { places, focused };
  }

  componentDidUpdate(_props: Props, prevState: State, before: Before | null): void {
    if (before) this.move(prevState, before);
    // A leaving element goes once its fade is over; one that came back stays.
    for (const entry of this.state.entries) {
      const timer = this.timers.get(entry.key);
      if (entry.leaving && !timer) {
        this.timers.set(
          entry.key,
          window.setTimeout(() => {
            this.timers.delete(entry.key);
            this.setState((state) => ({ entries: state.entries.filter((candidate) => !(candidate.key === entry.key && candidate.leaving)) }));
          }, MOTION_MS),
        );
      } else if (!entry.leaving && timer) {
        window.clearTimeout(timer);
        this.timers.delete(entry.key);
      }
    }
  }

  componentWillUnmount(): void {
    for (const timer of this.timers.values()) window.clearTimeout(timer);
    this.timers.clear();
  }

  /** Starts the motion of the picture that was just committed. */
  private move(prevState: State, before: Before): void {
    const previous = new Map(prevState.entries.map((entry) => [entry.key, entry]));
    // 1. What starts to leave is taken out of the flow where it was, and fades out.
    for (const entry of this.state.entries) {
      const element = this.elements.get(entry.key);
      if (!element) continue;
      const was = previous.get(entry.key);
      if (entry.leaving && was && !was.leaving) {
        const place = before.places.get(entry.key);
        if (place) Object.assign(element.style, { position: "absolute", top: `${place.top}px`, left: `${place.left}px`, width: `${place.width}px` });
        play(element, [{ opacity: 1 }, { opacity: 0 }], { fill: "forwards" });
      } else if (!entry.leaving && was?.leaving) {
        // It came back before it was gone.
        for (const property of ["position", "top", "left", "width"] as const) element.style.removeProperty(property);
        element.getAnimations?.().forEach((animation) => animation.cancel());
      }
    }
    // 2. What enters fades in; what stays and changed place slides from where it was.
    for (const entry of this.state.entries) {
      if (entry.leaving) continue;
      const element = this.elements.get(entry.key);
      if (!element) continue;
      const was = previous.get(entry.key);
      if (!was) {
        play(element, [{ opacity: 0 }, { opacity: 1 }]);
        continue;
      }
      const old = before.places.get(entry.key);
      if (!old || was.leaving) continue;
      const now = placeOf(element);
      const dx = old.left - now.left;
      const dy = old.top - now.top;
      if (dx !== 0 || dy !== 0) play(element, [{ transform: `translate(${dx}px, ${dy}px)` }, { transform: "none" }]);
    }
    // 3. A card that moved keeps the focus, whatever the browser does with a node that is moved.
    const { focused } = before;
    if (focused?.isConnected && !focused.closest("[inert]") && document.activeElement !== focused && (document.activeElement === null || document.activeElement === document.body)) {
      focused.focus({ preventScroll: true });
    }
  }

  render() {
    const { entries } = this.state;
    if (entries.length === 0) return null;
    return (
      <ul
        ref={(element) => {
          this.container = element;
        }}
        className="relative flex flex-col gap-3"
      >
        {entries.map((entry) => (
          <li
            key={entry.key}
            ref={(element) => {
              if (element) this.elements.set(entry.key, element);
              else this.elements.delete(entry.key);
            }}
            data-motion-key={entry.key}
            data-motion={entry.leaving ? "leaving" : undefined}
            inert={entry.leaving || undefined}
            aria-hidden={entry.leaving || undefined}
            className={entry.leaving ? "pointer-events-none" : undefined}
          >
            {entry.node}
          </li>
        ))}
      </ul>
    );
  }
}

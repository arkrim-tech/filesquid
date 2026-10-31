/**
 * The in-page half of the browsing performance probe (Task UI-F3.1, NFR-002 clock boundaries).
 * Playwright injects this file before any script of the app runs (`page.addInitScript({ path })`,
 * see `probe.ts`, which is the Node side and defines the shapes used here). It adds no behaviour
 * to the app: it only watches. For one armed sample it
 *
 * 1. takes the start from the first trusted input event of the sample (its `timeStamp`, the
 *    browser's own clock, on the same timeline as `performance.now()`), or from a fixed start
 *    (a cold load starts at navigation start, 0);
 * 2. polls the sample's predicates once per animation frame; a predicate is plain data, never
 *    code, so the page needs no `eval` and the app's own scripts stay untouched;
 * 3. ends at the start of the first animation frame after the frame in which a predicate first
 *    held, which is an upper bound of the moment the change was presented.
 *
 * It also records frame intervals for a scripted scroll, long tasks and element counts through
 * open shadow roots. When `window.__fsProbeArm` holds a sample spec (set by an earlier init
 * script), that sample is armed at once.
 */
(() => {
  if (window.__fsProbe) return;

  /** The element at a path of selectors joined by " >> ": each step looks in the light DOM, then in the open shadow root. */
  function find(path) {
    let context = document;
    for (const step of path.split(" >> ")) {
      if (context === null) return null;
      let hit = context.querySelector(step);
      const shadow = context.shadowRoot ?? null;
      if (hit === null && shadow !== null) hit = shadow.querySelector(step);
      context = hit;
    }
    return context;
  }

  /** All elements matching the last step of a path, inside the element the earlier steps name. */
  function findAll(path) {
    const steps = path.split(" >> ");
    const last = steps.pop();
    const parent = steps.length === 0 ? document.documentElement : find(steps.join(" >> "));
    if (parent === null) return [];
    const light = [...parent.querySelectorAll(last)];
    const shadow = parent.shadowRoot;
    return light.length > 0 || !shadow ? light : [...shadow.querySelectorAll(last)];
  }

  /** The element that has focus, looking through open shadow roots. */
  function deepActive() {
    let active = document.activeElement;
    while (active?.shadowRoot?.activeElement) active = active.shadowRoot.activeElement;
    return active;
  }

  /** Whether an attribute condition holds: present (no value), absent (null) or equal. */
  function attrHolds(el, pred) {
    if (pred.value === undefined) return el.hasAttribute(pred.name);
    if (pred.value === null) return !el.hasAttribute(pred.name);
    return el.getAttribute(pred.name) === pred.value;
  }

  /** Whether a text condition holds: equals and/or includes. */
  function textHolds(el, pred) {
    const text = (el.textContent ?? "").trim();
    if (pred.equals !== undefined && text !== pred.equals) return false;
    return pred.includes === undefined || text.includes(pred.includes);
  }

  /** The conditions that look at one element, by kind. */
  const ON_ELEMENT = {
    attr: attrHolds,
    text: textHolds,
    focus: (el) => deepActive() === el,
    value: (el, pred) => el.value === pred.equals,
    img: (el, pred) => el.complete && el.naturalWidth > 0 && el.alt === pred.alt,
  };

  /** Whether a predicate holds now. */
  function holds(pred) {
    if (pred.k === "all") return pred.of.every(holds);
    if (pred.k === "count") {
      const n = findAll(pred.at).length;
      return n >= (pred.min ?? 0) && n <= (pred.max ?? Infinity);
    }
    const el = find(pred.at);
    if (pred.k === "absent") return el === null;
    return el !== null && ON_ELEMENT[pred.k](el, pred);
  }

  /** The sample being measured; `spec` is null when none is armed. */
  const sample = {
    spec: null,
    start: null,
    ackAt: null,
    readyAt: null,
    ackSeen: false,
    readySeen: false,
    finished: null,
    timer: 0,
  };
  let done = Promise.resolve({ ackMs: null, readyMs: null, timedOut: true });
  const longTasks = [];

  /** Milestones of a cold load from the navigation and resource timing entries. */
  function milestones() {
    const out = {};
    const nav = performance.getEntriesByType("navigation")[0];
    if (nav) {
      out.responseEnd = nav.responseEnd;
      out.domContentLoaded = nav.domContentLoadedEventEnd;
    }
    const calls = performance
      .getEntriesByType("resource")
      .filter((e) => e.name.includes("/api/v1/"));
    const view = calls.find((e) => e.name.includes("/files/view"));
    if (view) {
      out.viewRequestStart = view.startTime;
      out.viewResponseEnd = view.responseEnd;
    }
    const listing = calls.find((e) => e.name.includes("/navigation"));
    if (listing) out.firstNavigationRequest = listing.startTime;
    const paint = performance.getEntriesByName("first-contentful-paint")[0];
    if (paint) out.firstContentfulPaint = paint.startTime;
    out.apiCalls = calls.length;
    return out;
  }

  /** The first trusted event of the armed types is the start of the sample. */
  function onEvent(event) {
    if (event.isTrusted && sample.start === null) sample.start = event.timeStamp;
  }

  /** End the sample with what was seen. */
  function finish(timedOut) {
    window.clearTimeout(sample.timer);
    for (const type of sample.spec?.events ?? []) window.removeEventListener(type, onEvent, true);
    const from = sample.start;
    const ack = sample.ackAt ?? sample.readyAt;
    const result = {
      ackMs: ack !== null && from !== null ? ack - from : null,
      readyMs: sample.readyAt !== null && from !== null ? sample.readyAt - from : null,
      timedOut,
    };
    if (sample.spec?.startAt !== undefined) result.milestones = milestones();
    sample.spec = null;
    sample.finished?.(result);
  }

  /** One animation frame of the sample: record what the previous frame showed, then look again. */
  function tick(frameStart) {
    if (sample.spec === null) return;
    if (sample.readySeen && sample.readyAt === null) {
      sample.readyAt = frameStart;
      return finish(false);
    }
    if (sample.ackSeen && sample.ackAt === null) sample.ackAt = frameStart;
    if (sample.start !== null) {
      if (!sample.ackSeen && sample.spec.ack !== undefined && holds(sample.spec.ack))
        sample.ackSeen = true;
      if (!sample.readySeen && holds(sample.spec.ready)) sample.readySeen = true;
    }
    requestAnimationFrame(tick);
  }

  /** Arm a sample. */
  function arm(spec) {
    Object.assign(sample, {
      spec,
      start: spec.startAt ?? null,
      ackAt: null,
      readyAt: null,
      ackSeen: false,
      readySeen: false,
    });
    done = new Promise((resolve) => (sample.finished = resolve));
    for (const type of spec.events) window.addEventListener(type, onEvent, true);
    sample.timer = window.setTimeout(() => finish(true), spec.timeoutMs);
    requestAnimationFrame(tick);
  }

  /** Start recording animation frame timestamps. */
  function framesStart() {
    const stamps = [];
    window.__frames = stamps;
    const step = (time) => {
      if (window.__frames !== stamps) return;
      stamps.push(time);
      requestAnimationFrame(step);
    };
    requestAnimationFrame(step);
  }

  /** The intervals between consecutive timestamps. */
  function intervalsOf(stamps) {
    return stamps.slice(1).map((time, i) => time - stamps[i]);
  }

  /** Stop recording and return the intervals between frames. */
  function framesStop() {
    const stamps = window.__frames ?? [];
    window.__frames = undefined;
    return intervalsOf(stamps);
  }

  /** One step of a scripted scroll: move the area by the distance this frame's time allows. */
  function scrollStep(run, time) {
    if (run.first === 0) run.first = run.last = time;
    const elapsed = time - run.first;
    const direction = run.reverse && elapsed > run.durationMs / 2 ? -1 : 1;
    run.area.scrollTop += (direction * run.pxPerSecond * (time - run.last)) / 1000;
    run.highest = Math.max(run.highest, run.area.scrollTop);
    run.last = time;
    run.stamps.push(time);
    return elapsed < run.durationMs;
  }

  /** Scroll an area at a constant speed, one step per animation frame, and report the frames. */
  function scrollRun(path, pxPerSecond, durationMs, reverse) {
    const area = find(path);
    if (area === null) return Promise.reject(new Error(`no scrolling area at ${path}`));
    const run = {
      area,
      pxPerSecond,
      durationMs,
      reverse,
      first: 0,
      last: 0,
      highest: 0,
      stamps: [],
    };
    const rows = `${path.split(" >> ").slice(0, -1).join(" >> ")} >> .row, .tile`;
    return new Promise((resolve) => {
      const frame = (time) => {
        if (scrollStep(run, time)) return void requestAnimationFrame(frame);
        resolve({
          intervals: intervalsOf(run.stamps),
          domRows: findAll(rows).length,
          maxScrollTop: run.highest,
          scrollHeight: area.scrollHeight,
        });
      };
      requestAnimationFrame(frame);
    });
  }

  /** Elements in the page, through open shadow roots. */
  function domCount() {
    let total = 0;
    const stack = [document];
    while (stack.length > 0) {
      for (const el of stack.pop().querySelectorAll("*")) {
        total += 1;
        if (el.shadowRoot) stack.push(el.shadowRoot);
      }
    }
    return total;
  }

  /** Scroll an area to its loaded end and report its visible and content heights. */
  function scrollToEnd(path) {
    const area = find(path);
    if (area === null) throw new Error(`no scrolling area at ${path}`);
    area.scrollTop = area.scrollHeight;
    return { clientHeight: area.clientHeight, scrollHeight: area.scrollHeight };
  }

  /** The centre of an element in viewport pixels, scrolled into view first when it is outside. */
  function centre(path) {
    const el = find(path);
    if (el === null) throw new Error(`nothing at ${path}`);
    let box = el.getBoundingClientRect();
    if (box.bottom > window.innerHeight || box.top < 0) {
      el.scrollIntoView({ block: "center" });
      box = el.getBoundingClientRect();
    }
    return { x: box.left + box.width / 2, y: box.top + box.height / 2 };
  }

  /** The link target and text of the element at a path, or null when nothing is there. */
  function describe(path) {
    const el = find(path);
    return el === null
      ? null
      : { href: el.getAttribute("href"), text: (el.textContent ?? "").trim() };
  }

  /** Scroll an area to a position, in pixels from the top. */
  function scrollTo(path, top) {
    const area = find(path);
    if (area === null) throw new Error(`no scrolling area at ${path}`);
    area.scrollTop = top;
  }

  window.__fsProbe = {
    arm,
    centre,
    describe,
    scrollTo,
    result: () => done,
    framesStart,
    framesStop,
    scrollRun,
    longTasks: () => longTasks.splice(0, longTasks.length),
    domCount,
    rowCount: (path) => findAll(path).length,
    scrollToEnd,
    now: () => performance.now(),
  };

  try {
    new PerformanceObserver((list) => {
      for (const entry of list.getEntries())
        longTasks.push({ start: entry.startTime, duration: entry.duration });
    }).observe({ type: "longtask", buffered: true });
  } catch (error) {
    console.info("long task observation is not available in this browser", error);
  }
  if (window.__fsProbeArm) arm(window.__fsProbeArm);
})();

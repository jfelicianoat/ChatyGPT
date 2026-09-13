type WallTime = {
  year: number;
  month: number;
  day: number;
  hour: number;
  minute: number;
  second: number;
};

const wallFormatter = (timezone: string) =>
  new Intl.DateTimeFormat("en-CA", {
    timeZone: timezone,
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hourCycle: "h23"
  });

function wallTime(value: Date, timezone: string): WallTime | null {
  try {
    const values = Object.fromEntries(
      wallFormatter(timezone)
        .formatToParts(value)
        .filter((part) => part.type !== "literal")
        .map((part) => [part.type, Number(part.value)])
    );
    const result = values as WallTime;
    return Object.values(result).every(Number.isFinite) ? result : null;
  } catch {
    return null;
  }
}

const two = (value: number) => String(value).padStart(2, "0");

export function scheduledTimeValueInZone(value: Date, timezone: string): string | null {
  const wall = wallTime(value, timezone);
  if (!wall) return null;
  return `${wall.year}-${two(wall.month)}-${two(wall.day)}T${two(wall.hour)}:${two(wall.minute)}`;
}

export function scheduledDateInZone(value: string, timezone: string): Date | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})$/.exec(value);
  if (!match) return null;
  const desired: WallTime = {
    year: Number(match[1]),
    month: Number(match[2]),
    day: Number(match[3]),
    hour: Number(match[4]),
    minute: Number(match[5]),
    second: 0
  };
  const desiredAsUtc = Date.UTC(
    desired.year,
    desired.month - 1,
    desired.day,
    desired.hour,
    desired.minute
  );
  let candidate = new Date(desiredAsUtc);
  for (let attempt = 0; attempt < 4; attempt += 1) {
    const observed = wallTime(candidate, timezone);
    if (!observed) return null;
    const observedAsUtc = Date.UTC(
      observed.year,
      observed.month - 1,
      observed.day,
      observed.hour,
      observed.minute,
      observed.second
    );
    const delta = desiredAsUtc - observedAsUtc;
    if (delta === 0) break;
    candidate = new Date(candidate.getTime() + delta);
  }
  const actual = wallTime(candidate, timezone);
  if (
    !actual ||
    actual.year !== desired.year ||
    actual.month !== desired.month ||
    actual.day !== desired.day ||
    actual.hour !== desired.hour ||
    actual.minute !== desired.minute
  ) {
    return null;
  }
  return candidate;
}

export function nextScheduledDateInZone(
  current: Date,
  timezone: string,
  days: number
): Date | null {
  const wall = wallTime(current, timezone);
  if (!wall) return null;
  const calendar = new Date(Date.UTC(wall.year, wall.month - 1, wall.day, wall.hour, wall.minute));
  calendar.setUTCDate(calendar.getUTCDate() + days);
  return scheduledDateInZone(
    `${calendar.getUTCFullYear()}-${two(calendar.getUTCMonth() + 1)}-${two(calendar.getUTCDate())}T${two(calendar.getUTCHours())}:${two(calendar.getUTCMinutes())}`,
    timezone
  );
}

export function scheduledDayKey(value: Date, timezone: string): string | null {
  const wall = wallTime(value, timezone);
  return wall ? `${wall.year}-${two(wall.month)}-${two(wall.day)}` : null;
}

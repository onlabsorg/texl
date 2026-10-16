# time

Timestamps and calendar dates. A timestamp is a number: the seconds elapsed
since 1 January 1970 at 00:00 UTC, possibly with a fraction. Every date is in
UTC.

## `time.now`

```text
time.now () -> Numb
```

Returns the current time as a timestamp. The argument is ignored.

```swan
time.now () > 1700000000       # => TRUE
```

## `time.to_date`

```text
time.to_date timestamp -> Namespace
```

Returns the calendar date of `timestamp` as a namespace with the members
`year`, `month` (1 to 12), `day` (1 to 31), `hour`, `minute` and `second`.
`second` keeps the fraction of the timestamp.

```swan
time.to_date 1780000000        # => {day: 28, hour: 20, minute: 26, month: 5, second: 40, year: 2026}
(time.to_date 0).year          # => 1970
```

## `time.from_date`

```text
time.from_date date -> Numb
```

Returns the timestamp of the date described by the namespace `date`, which can
have the members `year`, `month`, `day`, `hour`, `minute` and `second`.
Missing members default to year 0, month 1, day 1 and 00:00:00, so always give
at least the year.

```swan
time.from_date {year: 2026, month: 1, day: 1}      # => 1767225600
time.from_date {year: 1970}                         # => 0
```

## `time.to_string`

```text
time.to_string timestamp -> Text
```

Returns `timestamp` as an ISO 8601 text with milliseconds, such as
`2026-05-28T20:26:40.000Z`.

```swan
time.to_string 0                # => "1970-01-01T00:00:00.000Z"
time.to_string 1780000000.5     # => "2026-05-28T20:26:40.500Z"
```

## `time.from_string`

```text
time.from_string string -> Numb
```

Reads an ISO 8601 date and returns its timestamp. `string` is a date
`YYYY-MM-DD`, optionally followed by `T` (or a space) and a time `HH:MM` or
`HH:MM:SS`, with an optional fraction of a second and an optional final `Z`. A
missing time is midnight. Time zone offsets are not supported: the time is
always UTC. A text in another format gives `Undefined`.

```swan
time.from_string "2026-05-28"            # => 1779926400
time.from_string "2026-05-28T20:26:40Z"  # => 1780000000
time.from_string "28/05/2026"            # => Undefined
```

## `time.week_day`

```text
time.week_day timestamp -> Numb
```

Returns the day of the week of `timestamp`, from 0 (Sunday) to 6 (Saturday).

```swan
time.week_day 0                # => 4
time.week_day 1780000000       # => 4
```

## `time.week_number`

```text
time.week_number timestamp -> Numb
```

Returns the ISO 8601 week number of `timestamp`, from 1 to 53. ISO weeks start
on Monday, and week 1 is the week that contains the first Thursday of the
year.

```swan
time.week_number 1780000000    # => 22
time.week_number (time.from_string "2027-01-01")   # => 53
```

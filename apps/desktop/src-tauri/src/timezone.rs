//! Cálculo de recurrencias en la zona IANA elegida al programarlas.

use std::sync::OnceLock;

use chrono::{DateTime, Duration, NaiveDateTime, SecondsFormat, Utc};

use crate::error::AppError;

const DATA: &[u8] = include_bytes!("../resources/timezones.bin");

#[derive(Clone, Copy)]
struct Transition {
    at: i64,
    offset: i32,
}

struct ZoneDatabase {
    tables: Vec<Vec<Transition>>,
    zones: Vec<(String, usize)>,
}

static ZONES: OnceLock<Result<ZoneDatabase, String>> = OnceLock::new();

fn read<const N: usize>(data: &[u8], cursor: &mut usize) -> Result<[u8; N], String> {
    let end = cursor
        .checked_add(N)
        .ok_or_else(|| "desbordamiento al leer zonas horarias".to_owned())?;
    let bytes = data
        .get(*cursor..end)
        .ok_or_else(|| "datos de zonas horarias truncados".to_owned())?;
    *cursor = end;
    bytes
        .try_into()
        .map_err(|_| "datos de zonas horarias inválidos".to_owned())
}

fn parse_database() -> Result<ZoneDatabase, String> {
    if DATA.get(..4) != Some(b"CTZ1") {
        return Err("cabecera de zonas horarias inválida".to_owned());
    }
    let mut cursor = 4;
    let table_count = u32::from_le_bytes(read(DATA, &mut cursor)?) as usize;
    let mut tables = Vec::with_capacity(table_count);
    for _ in 0..table_count {
        let transition_count = u32::from_le_bytes(read(DATA, &mut cursor)?) as usize;
        let mut transitions = Vec::with_capacity(transition_count);
        for _ in 0..transition_count {
            transitions.push(Transition {
                at: i64::from_le_bytes(read(DATA, &mut cursor)?),
                offset: i32::from_le_bytes(read(DATA, &mut cursor)?),
            });
        }
        tables.push(transitions);
    }
    let zone_count = u32::from_le_bytes(read(DATA, &mut cursor)?) as usize;
    let mut zones = Vec::with_capacity(zone_count);
    for _ in 0..zone_count {
        let name_length = u16::from_le_bytes(read(DATA, &mut cursor)?) as usize;
        let end = cursor
            .checked_add(name_length)
            .ok_or_else(|| "desbordamiento al leer una zona horaria".to_owned())?;
        let name = std::str::from_utf8(
            DATA.get(cursor..end)
                .ok_or_else(|| "nombre de zona horaria truncado".to_owned())?,
        )
        .map_err(|_| "nombre de zona horaria inválido".to_owned())?
        .to_owned();
        cursor = end;
        let table = u32::from_le_bytes(read(DATA, &mut cursor)?) as usize;
        if table >= tables.len() {
            return Err("referencia de zona horaria inválida".to_owned());
        }
        zones.push((name, table));
    }
    if cursor != DATA.len() {
        return Err("sobran datos en la tabla de zonas horarias".to_owned());
    }
    Ok(ZoneDatabase { tables, zones })
}

fn transitions(timezone: &str) -> Result<&'static [Transition], AppError> {
    let database = ZONES
        .get_or_init(parse_database)
        .as_ref()
        .map_err(|error| AppError::Validation(error.clone()))?;
    let index = database
        .zones
        .binary_search_by(|(name, _)| name.as_str().cmp(timezone))
        .map_err(|_| AppError::Validation(format!("la zona horaria IANA {timezone} no existe")))?;
    Ok(&database.tables[database.zones[index].1])
}

pub fn validate(timezone: &str) -> Result<(), AppError> {
    transitions(timezone).map(|_| ())
}

fn offset_at(table: &[Transition], timestamp: i64) -> Result<i32, AppError> {
    let index = table.partition_point(|transition| transition.at <= timestamp);
    table
        .get(index.saturating_sub(1))
        .map(|transition| transition.offset)
        .ok_or_else(|| AppError::Validation("la zona horaria no cubre esa fecha".to_owned()))
}

fn local_to_utc(
    table: &[Transition],
    local: NaiveDateTime,
    previous_utc: i64,
) -> Result<i64, AppError> {
    let local_timestamp = local.and_utc().timestamp();
    let mut offsets = table
        .iter()
        .map(|transition| transition.offset)
        .collect::<Vec<_>>();
    offsets.sort_unstable();
    offsets.dedup();
    let mut candidates = offsets
        .into_iter()
        .map(|offset| local_timestamp - i64::from(offset))
        .filter(|candidate| {
            offset_at(table, *candidate)
                .is_ok_and(|offset| *candidate + i64::from(offset) == local_timestamp)
        })
        .collect::<Vec<_>>();
    candidates.sort_unstable();
    if let Some(candidate) = candidates
        .iter()
        .copied()
        .find(|candidate| *candidate > previous_utc)
        .or_else(|| candidates.first().copied())
    {
        return Ok(candidate);
    }

    for pair in table.windows(2) {
        let old_offset = pair[0].offset;
        let transition = pair[1];
        if transition.offset <= old_offset {
            continue;
        }
        let missing_from = transition.at + i64::from(old_offset);
        let missing_until = transition.at + i64::from(transition.offset);
        if (missing_from..missing_until).contains(&local_timestamp) {
            return Ok(local_timestamp - i64::from(old_offset));
        }
    }
    Err(AppError::Validation(
        "la zona horaria no permite calcular la siguiente fecha".to_owned(),
    ))
}

pub fn next_recurring_at(
    due_at: &str,
    timezone: &str,
    days: i64,
    now: DateTime<Utc>,
) -> Result<String, AppError> {
    let table = transitions(timezone)?;
    let mut occurrence = DateTime::parse_from_rfc3339(due_at)
        .map_err(|_| AppError::Validation("la fecha programada no es válida".to_owned()))?
        .with_timezone(&Utc);
    for _ in 0..5000 {
        if occurrence > now {
            return Ok(occurrence.to_rfc3339_opts(SecondsFormat::Millis, true));
        }
        let utc_timestamp = occurrence.timestamp();
        let local =
            occurrence.naive_utc() + Duration::seconds(i64::from(offset_at(table, utc_timestamp)?));
        let next_local = local
            .checked_add_signed(Duration::days(days))
            .ok_or_else(|| AppError::Validation("la recurrencia sale del calendario".to_owned()))?;
        let next_timestamp = local_to_utc(table, next_local, utc_timestamp)?;
        occurrence = DateTime::from_timestamp(next_timestamp, 0).ok_or_else(|| {
            AppError::Validation("la siguiente fecha programada no es válida".to_owned())
        })?;
    }
    Err(AppError::Conflict(
        "la programación lleva más de 5.000 recurrencias de retraso; edítala para elegir una fecha próxima"
            .to_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canary_daily_recurrence_keeps_wall_clock_time_across_dst() {
        let spring = next_recurring_at(
            "2026-03-28T10:00:00.000Z",
            "Atlantic/Canary",
            1,
            DateTime::parse_from_rfc3339("2026-03-28T10:00:00.000Z")
                .unwrap()
                .with_timezone(&Utc),
        )
        .unwrap();
        assert_eq!(spring, "2026-03-29T09:00:00.000Z");

        let autumn = next_recurring_at(
            "2026-10-24T09:00:00.000Z",
            "Atlantic/Canary",
            1,
            DateTime::parse_from_rfc3339("2026-10-24T09:00:00.000Z")
                .unwrap()
                .with_timezone(&Utc),
        )
        .unwrap();
        assert_eq!(autumn, "2026-10-25T10:00:00.000Z");
    }
}

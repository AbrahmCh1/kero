//! Fechas civiles y zonas horarias sin dependencias.
//!
//! Los desfases los da el sistema: en Windows, la ICU que trae (se carga la
//! primera vez que se pide una hora en otra ciudad); en macOS y Linux, los
//! archivos de `/usr/share/zoneinfo`.

/// Días desde 1970-01-01 (algoritmo de H. Hinnant).
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// (año, mes, día) de un número de días desde 1970-01-01.
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// 0 = domingo.
pub fn weekday(days: i64) -> u32 {
    (days + 4).rem_euclid(7) as u32
}

pub fn is_leap(y: i64) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}

pub fn days_in_month(y: i64, m: u32) -> u32 {
    match m {
        2 if is_leap(y) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Semana ISO 8601 (1–53).
pub fn iso_week(days: i64) -> u32 {
    let wd = (weekday(days) + 6) % 7; // 0 = lunes
    let thursday = days - wd as i64 + 3;
    let (y, _, _) = civil_from_days(thursday);
    ((thursday - days_from_civil(y, 1, 1)) / 7 + 1) as u32
}

pub fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Desfase local respecto a UTC (segundos) en el instante `utc`.
pub fn local_offset(utc: i64) -> i32 {
    sys::offset(None, utc).unwrap_or(0)
}

/// Desfase de la zona IANA `id` ("Asia/Tokyo") en el instante `utc`.
pub fn offset(id: &str, utc: i64) -> Option<i32> {
    match id {
        "UTC" => Some(0),
        _ => sys::offset(Some(id), utc),
    }
}

/// Nombre IANA de la zona local, si el sistema lo dice.
pub fn local_zone() -> Option<String> {
    sys::local_zone()
}

// ---------------------------------------------------------------------------
// Windows: ICU del sistema (icu.dll desde Windows 10 1903; antes icuin.dll).

#[cfg(windows)]
mod sys {
    use std::ffi::c_void;
    use std::sync::OnceLock;
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
    use windows::core::{PCSTR, w};

    type Open = unsafe extern "C" fn(*const u16, i32, *const u8, i32, *mut i32) -> *mut c_void;
    type SetMillis = unsafe extern "C" fn(*mut c_void, f64, *mut i32);
    type Get = unsafe extern "C" fn(*const c_void, i32, *mut i32) -> i32;
    type Close = unsafe extern "C" fn(*mut c_void);
    type DefaultZone = unsafe extern "C" fn(*mut u16, i32, *mut i32) -> i32;
    type Proc = unsafe extern "system" fn() -> isize;
    type Canonical = unsafe extern "C" fn(*const u16, i32, *mut u16, i32, *mut i8, *mut i32) -> i32;

    struct Icu {
        open: Open,
        set_millis: SetMillis,
        get: Get,
        close: Close,
        default_zone: DefaultZone,
        canonical: Canonical,
    }

    const UCAL_GREGORIAN: i32 = 1;
    const UCAL_ZONE_OFFSET: i32 = 15;
    const UCAL_DST_OFFSET: i32 = 16;

    fn icu() -> Option<&'static Icu> {
        static ICU: OnceLock<Option<Icu>> = OnceLock::new();
        ICU.get_or_init(|| unsafe {
            let lib = LoadLibraryW(w!("icu.dll")).or_else(|_| LoadLibraryW(w!("icuin.dll"))).ok()?;
            let f = |name: &[u8]| GetProcAddress(lib, PCSTR(name.as_ptr()));
            Some(Icu {
                open: std::mem::transmute::<Proc, Open>(f(b"ucal_open\0")?),
                set_millis: std::mem::transmute::<Proc, SetMillis>(f(b"ucal_setMillis\0")?),
                get: std::mem::transmute::<Proc, Get>(f(b"ucal_get\0")?),
                close: std::mem::transmute::<Proc, Close>(f(b"ucal_close\0")?),
                default_zone: std::mem::transmute::<Proc, DefaultZone>(f(b"ucal_getDefaultTimeZone\0")?),
                canonical: std::mem::transmute::<Proc, Canonical>(f(b"ucal_getCanonicalTimeZoneID\0")?),
            })
        })
        .as_ref()
    }

    pub fn offset(id: Option<&str>, utc: i64) -> Option<i32> {
        let icu = icu()?;
        let wide: Vec<u16> = id.unwrap_or("").encode_utf16().collect();
        unsafe {
            let mut err = 0i32;
            if id.is_some() {
                // ICU acepta cualquier nombre y cae a GMT: hay que comprobar que exista.
                let mut buf = [0u16; 64];
                let mut system = 0i8;
                (icu.canonical)(wide.as_ptr(), wide.len() as i32, buf.as_mut_ptr(), 64, &mut system, &mut err);
                if err > 0 || system == 0 {
                    return None;
                }
            }
            let (ptr, len) = if id.is_some() { (wide.as_ptr(), wide.len() as i32) } else { (std::ptr::null(), 0) };
            let cal = (icu.open)(ptr, len, std::ptr::null(), UCAL_GREGORIAN, &mut err);
            if cal.is_null() || err > 0 {
                return None;
            }
            (icu.set_millis)(cal, utc as f64 * 1000.0, &mut err);
            let zone = (icu.get)(cal, UCAL_ZONE_OFFSET, &mut err);
            let dst = (icu.get)(cal, UCAL_DST_OFFSET, &mut err);
            (icu.close)(cal);
            (err <= 0).then_some((zone + dst) / 1000)
        }
    }

    pub fn local_zone() -> Option<String> {
        let icu = icu()?;
        let mut buf = [0u16; 64];
        let mut err = 0i32;
        let n = unsafe { (icu.default_zone)(buf.as_mut_ptr(), 64, &mut err) };
        (err <= 0 && n > 0).then(|| String::from_utf16_lossy(&buf[..n as usize]))
    }
}

// ---------------------------------------------------------------------------
// macOS y Linux: archivos TZif de zoneinfo.

#[cfg(unix)]
mod sys {
    use super::tzif::Zone;
    use parking_lot::Mutex;
    use std::collections::HashMap;
    use std::path::PathBuf;

    const DIRS: &[&str] = &["/usr/share/zoneinfo", "/var/db/timezone/zoneinfo", "/usr/lib/zoneinfo"];

    fn load(id: Option<&str>) -> Option<Zone> {
        let path = match id {
            Some(id) => {
                if id.contains("..") || id.starts_with('/') {
                    return None;
                }
                DIRS.iter().map(|d| PathBuf::from(d).join(id)).find(|p| p.is_file())?
            }
            None => match std::env::var("TZ").ok().map(|t| t.trim_start_matches(':').to_string()) {
                Some(t) if t.starts_with('/') => PathBuf::from(t),
                Some(t) if !t.is_empty() => return load(Some(&t)),
                _ => PathBuf::from("/etc/localtime"),
            },
        };
        Zone::parse(&std::fs::read(path).ok()?)
    }

    /// Las zonas consultadas se guardan: son pocas y pequeñas (~1 KB).
    pub fn offset(id: Option<&str>, utc: i64) -> Option<i32> {
        static CACHE: Mutex<Option<HashMap<String, Option<Zone>>>> = Mutex::new(None);
        let mut guard = CACHE.lock();
        let cache = guard.get_or_insert_with(HashMap::new);
        let key = id.unwrap_or("").to_string();
        let zone = cache.entry(key).or_insert_with(|| load(id));
        zone.as_ref().map(|z| z.offset(utc))
    }

    pub fn local_zone() -> Option<String> {
        if let Ok(t) = std::env::var("TZ") {
            let t = t.trim_start_matches(':');
            if !t.is_empty() && !t.starts_with('/') {
                return Some(t.to_string());
            }
        }
        let target = std::fs::read_link("/etc/localtime").ok()?;
        let s = target.to_string_lossy();
        s.split_once("zoneinfo/").map(|(_, id)| id.to_string())
    }
}

#[cfg(any(unix, test))]
#[cfg_attr(not(unix), allow(dead_code))]
mod tzif {
    use super::{days_from_civil, days_in_month, weekday};

    /// Regla POSIX del pie de un TZif ("CST6CDT,M3.2.0,M11.1.0").
    #[derive(Debug, Clone, PartialEq)]
    pub struct Rule {
        std: i32,
        dst: Option<(i32, Date, i32, Date, i32)>, // (desfase, inicio, hora, fin, hora)
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    enum Date {
        /// Mes, semana (5 = última), día de la semana (0 = domingo).
        M(u32, u32, u32),
        /// Día juliano 1–365 sin contar el 29 de febrero.
        J(u32),
        /// Día 0–365 contando el 29 de febrero.
        N(u32),
    }

    pub struct Zone {
        times: Vec<i64>,
        idx: Vec<u8>,
        offsets: Vec<i32>,
        rule: Option<Rule>,
    }

    fn be32(b: &[u8], at: usize) -> Option<u32> {
        Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
    }

    impl Zone {
        pub fn parse(b: &[u8]) -> Option<Zone> {
            if b.get(..4)? != b"TZif" {
                return None;
            }
            let counts = |at: usize| -> Option<[usize; 6]> {
                let mut c = [0usize; 6];
                for (i, v) in c.iter_mut().enumerate() {
                    *v = be32(b, at + 20 + i * 4)? as usize;
                }
                Some(c)
            };
            let v1 = counts(0)?;
            let v1_len = |c: [usize; 6], t: usize| c[3] * t + c[3] + c[4] * 6 + c[5] + c[2] * (t + 4) + c[0] + c[1];
            let (base, c, t) = if b[4] >= b'2' {
                let second = 44 + v1_len(v1, 4);
                (second, counts(second)?, 8)
            } else {
                (0, v1, 4)
            };
            let [_, _, _, timecnt, typecnt, _] = c;
            let mut p = base + 44;
            let mut times = Vec::with_capacity(timecnt);
            for i in 0..timecnt {
                let at = p + i * t;
                times.push(if t == 8 {
                    i64::from_be_bytes(b.get(at..at + 8)?.try_into().ok()?)
                } else {
                    be32(b, at)? as i32 as i64
                });
            }
            p += timecnt * t;
            let idx = b.get(p..p + timecnt)?.to_vec();
            p += timecnt;
            let mut offsets = Vec::with_capacity(typecnt);
            for i in 0..typecnt {
                offsets.push(be32(b, p + i * 6)? as i32);
            }
            if offsets.is_empty() || idx.iter().any(|&i| i as usize >= offsets.len()) {
                return None;
            }
            let rule = if t == 8 {
                let footer = b.get(base + 44 + v1_len(c, 8)..).unwrap_or_default();
                std::str::from_utf8(footer).ok().and_then(|s| parse_rule(s.trim_matches('\n')))
            } else {
                None
            };
            Some(Zone { times, idx, offsets, rule })
        }

        pub fn offset(&self, utc: i64) -> i32 {
            let n = self.times.partition_point(|&t| t <= utc);
            // Después de la última transición manda la regla del pie (los TZif "slim" casi sólo traen eso).
            if n == self.times.len()
                && let Some(rule) = &self.rule
            {
                return rule.offset(utc);
            }
            match n {
                0 => self.offsets[0],
                n => self.offsets[self.idx[n - 1] as usize],
            }
        }
    }

    /// Nombre ("CST", "<-03>"), desfase POSIX ("6", "-5:30") con el signo al revés.
    fn take_name(s: &str) -> Option<&str> {
        if let Some(rest) = s.strip_prefix('<') {
            return Some(&rest[rest.find('>')? + 1..]);
        }
        let n = s.bytes().take_while(|b| b.is_ascii_alphabetic()).count();
        (n >= 3).then(|| &s[n..])
    }

    fn take_time(s: &str) -> Option<(i32, &str)> {
        let (sign, s) = match s.as_bytes().first()? {
            b'-' => (-1, &s[1..]),
            b'+' => (1, &s[1..]),
            _ => (1, s),
        };
        let n = s.bytes().take_while(|b| b.is_ascii_digit() || *b == b':').count();
        if n == 0 {
            return None;
        }
        let mut secs = 0;
        for (i, part) in s[..n].split(':').enumerate().take(3) {
            secs += part.parse::<i32>().ok()? * [3600, 60, 1][i];
        }
        Some((sign * secs, &s[n..]))
    }

    fn take_date(s: &str) -> Option<(Date, i32, &str)> {
        let (date, rest) = if let Some(r) = s.strip_prefix('M') {
            let n = r.find(|c: char| c != '.' && !c.is_ascii_digit()).unwrap_or(r.len());
            let mut it = r[..n].split('.').map(|p| p.parse::<u32>().ok());
            (Date::M(it.next()??, it.next()??, it.next()??), &r[n..])
        } else {
            let (julian, r) = match s.strip_prefix('J') {
                Some(r) => (true, r),
                None => (false, s),
            };
            let n = r.bytes().take_while(u8::is_ascii_digit).count();
            let d = r[..n].parse().ok()?;
            (if julian { Date::J(d) } else { Date::N(d) }, &r[n..])
        };
        match rest.strip_prefix('/') {
            Some(r) => {
                let (time, r) = take_time(r)?;
                Some((date, time, r))
            }
            None => Some((date, 7200, rest)),
        }
    }

    pub fn parse_rule(s: &str) -> Option<Rule> {
        let rest = take_name(s)?;
        let (std, rest) = take_time(rest)?;
        let std = -std;
        if rest.is_empty() {
            return Some(Rule { std, dst: None });
        }
        let rest = take_name(rest)?;
        let (dst, rest) = match take_time(rest) {
            Some((t, r)) if !rest.starts_with(',') => (-t, r),
            _ => (std + 3600, rest),
        };
        let rest = rest.strip_prefix(',')?;
        let (start, start_t, rest) = take_date(rest)?;
        let rest = rest.strip_prefix(',')?;
        let (end, end_t, _) = take_date(rest)?;
        Some(Rule { std, dst: Some((dst, start, start_t, end, end_t)) })
    }

    fn day_of(year: i64, d: Date) -> i64 {
        match d {
            Date::M(m, w, wd) => {
                let first = days_from_civil(year, m, 1);
                let mut day = 1 + (wd + 7 - weekday(first)) % 7 + (w.saturating_sub(1)) * 7;
                while day > days_in_month(year, m) {
                    day -= 7;
                }
                first + day as i64 - 1
            }
            Date::J(n) => {
                let jan1 = days_from_civil(year, 1, 1);
                let leap_shift = super::is_leap(year) && n >= 60;
                jan1 + n as i64 - 1 + i64::from(leap_shift)
            }
            Date::N(n) => days_from_civil(year, 1, 1) + n as i64,
        }
    }

    impl Rule {
        pub fn offset(&self, utc: i64) -> i32 {
            let Some((dst, start, start_t, end, end_t)) = self.dst else { return self.std };
            let local = utc + self.std as i64;
            let (year, _, _) = super::civil_from_days(local.div_euclid(86_400));
            let begins = day_of(year, start) * 86_400 + start_t as i64 - self.std as i64;
            let ends = day_of(year, end) * 86_400 + end_t as i64 - dst as i64;
            let in_dst = if begins < ends { (begins..ends).contains(&utc) } else { !(ends..begins).contains(&utc) };
            if in_dst { dst } else { self.std }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn posix_rules() {
            let ny = parse_rule("EST5EDT,M3.2.0,M11.1.0").unwrap();
            // 2026-07-01 12:00 UTC → EDT (-4); 2026-12-01 → EST (-5).
            assert_eq!(ny.offset(days_from_civil(2026, 7, 1) * 86_400 + 43_200), -4 * 3600);
            assert_eq!(ny.offset(days_from_civil(2026, 12, 1) * 86_400), -5 * 3600);
            // Cambio: 2026-03-08 02:00 EST = 07:00 UTC.
            let change = days_from_civil(2026, 3, 8) * 86_400 + 7 * 3600;
            assert_eq!(ny.offset(change - 1), -5 * 3600);
            assert_eq!(ny.offset(change), -4 * 3600);
            // Hemisferio sur: Santiago de Chile.
            let scl = parse_rule("<-04>4<-03>,M9.1.6/24,M4.1.6/24").unwrap();
            assert_eq!(scl.offset(days_from_civil(2026, 1, 15) * 86_400), -3 * 3600);
            assert_eq!(scl.offset(days_from_civil(2026, 6, 15) * 86_400), -4 * 3600);
            assert_eq!(parse_rule("JST-9").unwrap().offset(0), 9 * 3600);
            assert_eq!(parse_rule("<+0530>-5:30").unwrap().offset(0), 5 * 3600 + 1800);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(civil_from_days(days_from_civil(2026, 11, 3)), (2026, 11, 3));
        assert_eq!(civil_from_days(days_from_civil(2000, 2, 29)), (2000, 2, 29));
        assert_eq!(weekday(days_from_civil(2026, 11, 3)), 2); // martes
        assert_eq!(iso_week(days_from_civil(2026, 1, 1)), 1);
        assert_eq!(iso_week(days_from_civil(2027, 1, 1)), 53);
    }

    #[test]
    fn system_zones() {
        // Tokio no tiene horario de verano; Nueva York sí.
        let t = days_from_civil(2026, 7, 1) * 86_400;
        assert_eq!(offset("Asia/Tokyo", t), Some(9 * 3600));
        assert_eq!(offset("America/New_York", t), Some(-4 * 3600));
        assert_eq!(offset("America/New_York", days_from_civil(2026, 12, 1) * 86_400), Some(-5 * 3600));
        assert_eq!(offset("Narnia/Cair_Paravel", t), None);
    }
}

//! Fournisseurs d'information des segments (`term_stat`, `term_prompt`,
//! `term_context`).
//!
//! Tout est best-effort : une information indisponible renvoie une chaîne vide
//! plutôt que de faire échouer l'affichage.

use std::ffi::CStr;
use std::net::{IpAddr, UdpSocket};
use std::sync::OnceLock;
use std::time::Instant;

/// Nom d'utilisateur courant.
pub fn utilisateur() -> String
  {
    std::env::var("USER")
      .or_else(|_| std::env::var("LOGNAME"))
      .or_else(|_| std::env::var("USERNAME"))
      .unwrap_or_else(|_| "?".to_string())
  }

/// Nom d'hôte court.
pub fn hote() -> String
  {
    if let Ok(nom) = std::env::var("HOSTNAME")
      && !nom.is_empty()
      {
        return nom;
      }
    if let Ok(contenu) = std::fs::read_to_string("/proc/sys/kernel/hostname")
      {
        let nom = contenu.trim();
        if !nom.is_empty()
          {
            return nom.to_string();
          }
      }
    if let Ok(contenu) = std::fs::read_to_string("/etc/hostname")
      {
        let nom = contenu.trim();
        if !nom.is_empty()
          {
            return nom.to_string();
          }
      }
    "localhost".to_string()
  }

/// Nom d'hôte pleinement qualifié (FQDN), repli sur le nom court.
pub fn fqdn() -> String
  {
    if let Ok(sortie) = std::process::Command::new("hostname").arg("-f").output()
      && sortie.status.success()
      {
        let nom = String::from_utf8_lossy(&sortie.stdout).trim().to_string();
        if !nom.is_empty() && !nom.contains(' ') && !nom.contains('\t') && nom.contains('.')
          {
            return nom;
          }
      }
    if let Ok(sortie) = std::process::Command::new("hostname").arg("-d").output()
      && sortie.status.success()
      {
        let domaine = String::from_utf8_lossy(&sortie.stdout).trim().to_string();
        if domaine_valide(&domaine)
          {
            return format!("{}.{}", hote(), domaine);
          }
      }
    if let Some(domaine) = domaine()
      {
        let hote = hote();
        if hote.contains('.')
          {
            return hote;
          }
        return format!("{hote}.{domaine}");
      }
    if let Some(ip) = adresse_ip()
      && let Some(revers) = fqdn_par_revers(&ip)
      {
        return revers;
      }
    hote()
  }

/// Résolution DNS inverse native (PTR), sans commande externe (`dig` absent).
///
/// Utilise `getnameinfo` (résolveur système) côté Unix : indépendant du système
/// et de la plateforme. Le résultat est mis en cache pour la session.
pub fn fqdn_par_revers(ip: &str) -> Option<String>
  {
    static CACHE: OnceLock<Option<String>> = OnceLock::new();
    CACHE.get_or_init(|| {
      let nom = resolver_ptrs(ip)?;
      let nom = nom.trim().trim_end_matches('.').to_string();
      let adresse_ok = ip.parse::<IpAddr>().is_ok() && nom.as_str() != ip;
      if nom.is_empty() || !nom.contains('.') || !adresse_ok
        {
          None
        }
      else
        {
          Some(nom)
        }
    })
    .clone()
  }

/// Interroge le résolveur local pour l'enregistrement PTR de `ip`.
#[cfg(unix)]
fn resolver_ptrs(ip: &str) -> Option<String>
  {
    let adresse = ip.parse::<IpAddr>().ok()?;
    let (pointeur, longueur): (*const libc::sockaddr, libc::socklen_t) = match adresse
      {
        std::net::IpAddr::V4(v4) => {
          let sock = libc::sockaddr_in {
            sin_family: libc::AF_INET as libc::sa_family_t,
            sin_port: 0,
            sin_addr: libc::in_addr { s_addr: u32::from_ne_bytes(v4.octets()) },
            sin_zero: [0; 8],
          };
          (
            &sock as *const libc::sockaddr_in as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
          )
        }
        std::net::IpAddr::V6(v6) => {
          let sock = libc::sockaddr_in6 {
            sin6_family: libc::AF_INET6 as libc::sa_family_t,
            sin6_port: 0,
            sin6_flowinfo: 0,
            sin6_addr: libc::in6_addr { s6_addr: v6.octets() },
            sin6_scope_id: 0,
          };
          (
            &sock as *const libc::sockaddr_in6 as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_in6>() as libc::socklen_t,
          )
        }
      };
    let mut tampon = vec![0u8; libc::NI_MAXHOST as usize];
    let code = unsafe {
      libc::getnameinfo(
        pointeur,
        longueur,
        tampon.as_mut_ptr() as *mut libc::c_char,
        tampon.len() as libc::socklen_t,
        std::ptr::null_mut(),
        0,
        libc::NI_NAMEREQD,
      )
    };
    if code != 0
      {
        return None;
      }
    CStr::from_bytes_until_nul(&tampon)
      .ok()
      .and_then(|cstr| cstr.to_str().ok().map(|n| n.to_string()))
  }

/// Résolution DNS inverse non disponible hors Unix.
#[cfg(not(unix))]
fn resolver_ptrs(_ip: &str) -> Option<String>
  {
    None
  }

/// Indique si une chaîne est un nom de domaine plausible.
fn domaine_valide(domaine: &str) -> bool
  {
    !domaine.is_empty() && !domaine.contains(' ') && domaine != "(none)" && domaine != "localdomain"
  }

/// Domaine DNS local (`dnsdomainname`, puis `/etc/resolv.conf`).
fn domaine() -> Option<String>
  {
    if let Ok(sortie) = std::process::Command::new("dnsdomainname").output()
      && sortie.status.success()
      {
        let domaine = String::from_utf8_lossy(&sortie.stdout).trim().to_string();
        if domaine_valide(&domaine)
          {
            return Some(domaine);
          }
      }
    if let Ok(contenu) = std::fs::read_to_string("/etc/resolv.conf")
      {
        for ligne in contenu.lines()
          {
            let mut mots = ligne.split_whitespace();
            if let Some("domain" | "search") = mots.next()
              && let Some(domaine) = mots.next()
              && domaine_valide(domaine)
              {
                return Some(domaine.to_string());
              }
          }
      }
    None
  }

/// Adresse IP locale (aucun paquet émis : on ne fait que lier puis connecter).
pub fn adresse_ip() -> Option<String>
  {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok().map(|adresse| adresse.ip().to_string())
  }

/// Identité : `user@host`.
pub fn identite() -> String
  {
    format!("{}@{}", utilisateur(), hote())
  }

/// Nom d'OS et flavor lisibles.
pub fn os_flavor() -> String
  {
    let os = std::env::consts::OS;
    if os == "linux"
      && let Ok(os_release) = std::fs::read_to_string("/etc/os-release")
      {
        for ligne in os_release.lines()
          {
            if let Some(nom) = ligne.strip_prefix("PRETTY_NAME=")
              {
                return nom.trim_matches('"').to_string();
              }
          }
      }
    format!("{} ({})", os, std::env::consts::ARCH)
  }

/// Code pays déduit de la locale (`fr_FR` → `FR`).
pub fn code_pays() -> String
  {
    let locale = std::env::var("LANG").or_else(|_| std::env::var("LC_ALL")).unwrap_or_default();
    let locale = locale.split('.').next().unwrap_or("");
    if let Some((_, pays)) = locale.split_once('_')
      {
        let pays = pays.split('@').next().unwrap_or("").to_ascii_uppercase();
        if pays.len() == 2 && pays.chars().all(|c| c.is_ascii_alphabetic())
          {
            return pays;
          }
      }
    "??".to_string()
  }

/// Date et heure locales (complètes) et UTC (jour mois heure minute).
pub fn horloge() -> (String, String)
  {
    let local_format = time::macros::format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");
    let utc_format = time::macros::format_description!("[day]/[month] [hour]:[minute]");
    let maintenant = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    let local = maintenant.format(&local_format).unwrap_or_default();
    let utc = maintenant.to_offset(time::UtcOffset::UTC).format(&utc_format).unwrap_or_default();
    (local, utc)
  }

/// Drapeau (indicateurs régionaux Unicode) d'un code pays à deux lettres.
pub fn drapeau(code: &str) -> String
  {
    let code = code.trim().to_ascii_uppercase();
    let lettres: Vec<char> = code.chars().collect();
    if lettres.len() != 2 || !lettres.iter().all(char::is_ascii_uppercase)
      {
        return String::new();
      }
    let mut drapeau = String::new();
    for lettre in lettres
      {
        let point = 0x1F1E6u32 + (lettre as u32 - 'A' as u32);
        if let Some(symbole) = char::from_u32(point)
          {
            drapeau.push(symbole);
          }
      }
    drapeau
  }

/// Charge système sur une minute (compacte), lue dans `/proc/loadavg`.
pub fn charge() -> String
  {
    if let Ok(contenu) = std::fs::read_to_string("/proc/loadavg")
      && let Some(premier) = contenu.split_whitespace().next()
      {
        return premier.to_string();
      }
    String::new()
  }

/// Mesure compacte cpu/mémoire/io, rafraîchie périodiquement.
///
/// Les débits sont calculés par différence entre deux échantillons successifs.
pub struct MesureSysteme
  {
    precedent: Option<Echantillon>,
    pub texte: String,
  }

struct Echantillon
  {
    cpu_total: u64,
    cpu_inactif: u64,
    io_octets: u64,
    instant: Instant,
  }

impl MesureSysteme
  {
    /// Crée une mesure et effectue un premier échantillonnage.
    pub fn nouveau() -> Self
      {
        let mut mesure = Self { precedent: None, texte: String::new() };
        mesure.maj();
        mesure
      }

    /// Rééchantillonne et met à jour le texte affiché.
    pub fn maj(&mut self)
      {
        let maintenant = Instant::now();
        let (cpu_total, cpu_inactif) = lire_cpu();
        let io_octets = lire_io();
        if let Some(precedent) = &self.precedent
          {
            let delta_total = cpu_total.saturating_sub(precedent.cpu_total);
            let delta_inactif = cpu_inactif.saturating_sub(precedent.cpu_inactif);
            let delta_io = io_octets.saturating_sub(precedent.io_octets);
            let secondes = maintenant.duration_since(precedent.instant).as_secs_f64().max(0.001);
            let cpu = if delta_total > 0
              {
                100.0 * (1.0 - delta_inactif as f64 / delta_total as f64)
              }
            else
              {
                0.0
              };
            let debit = delta_io as f64 / secondes;
            let memoire = lire_memoire();
            self.texte = format!("CPU {:>3.0}%  MEM {:>2}%  IO {}", cpu, memoire, debit_lisible(debit));
          }
        self.precedent = Some(Echantillon { cpu_total, cpu_inactif, io_octets, instant: maintenant });
      }
  }

fn lire_cpu() -> (u64, u64)
  {
    let Ok(contenu) = std::fs::read_to_string("/proc/stat") else
      {
        return (0, 0);
      };
    let Some(ligne) = contenu.lines().next() else
      {
        return (0, 0);
      };
    let champs: Vec<u64> = ligne.split_whitespace().skip(1).filter_map(|v| v.parse().ok()).collect();
    let total: u64 = champs.iter().sum();
    let inactif = champs.get(3).copied().unwrap_or(0) + champs.get(4).copied().unwrap_or(0);
    (total, inactif)
  }

fn lire_memoire() -> u64
  {
    let Ok(contenu) = std::fs::read_to_string("/proc/meminfo") else
      {
        return 0;
      };
    let mut total = 0u64;
    let mut disponible = 0u64;
    for ligne in contenu.lines()
      {
        if let Some(valeur) = ligne.strip_prefix("MemTotal:")
          {
            total = valeur.split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(0);
          }
        else if let Some(valeur) = ligne.strip_prefix("MemAvailable:")
          {
            disponible = valeur.split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(0);
          }
      }
    total.saturating_sub(disponible)
      .saturating_mul(100)
      .checked_div(total)
      .map_or(0, |pourcentage| pourcentage.min(100))
  }

fn lire_io() -> u64
  {
    let Ok(contenu) = std::fs::read_to_string("/proc/diskstats") else
      {
        return 0;
      };
    let mut octets = 0u64;
    for ligne in contenu.lines()
      {
        let champs: Vec<&str> = ligne.split_whitespace().collect();
        if champs.len() < 10
          {
            continue;
          }
        let lus = champs[5].parse::<u64>().unwrap_or(0);
        let ecrits = champs[9].parse::<u64>().unwrap_or(0);
        octets = octets.saturating_add((lus.saturating_add(ecrits)).saturating_mul(512));
      }
    octets
  }

fn debit_lisible(octets_par_seconde: f64) -> String
  {
    let unites = ["o/s", "Ko/s", "Mo/s", "Go/s"];
    let mut valeur = octets_par_seconde;
    let mut rang = 0;
    while valeur >= 1024.0 && rang < unites.len() - 1
      {
        valeur /= 1024.0;
        rang += 1;
      }
    if rang == 0
      {
        format!("{valeur:.0} {}", unites[rang])
      }
    else
      {
        format!("{valeur:.1} {}", unites[rang])
      }
  }

/// Masque les données sensibles d'une chaîne (mode capture).
///
/// Masque les adresses IPv4, les adresses MAC et les affectations de secrets.
pub fn masquer(texte: &str) -> String
  {
    let masque_ip = masquer_ipv4(texte);
    let masque_mac = masquer_mac(&masque_ip);
    masquer_secrets(&masque_mac)
  }

fn masquer_ipv4(texte: &str) -> String
  {
    let caracteres: Vec<char> = texte.chars().collect();
    let mut sortie = String::with_capacity(texte.len());
    let mut i = 0;
    while i < caracteres.len()
      {
        if let Some(longueur) = essayer_ipv4(&caracteres[i..])
          {
            sortie.push_str("x.x.x.x");
            i += longueur;
          }
        else
          {
            sortie.push(caracteres[i]);
            i += 1;
          }
      }
    sortie
  }

fn essayer_ipv4(caracteres: &[char]) -> Option<usize>
  {
    let mut i = 0;
    for groupe in 0..4
      {
        let debut = i;
        let mut valeur = 0u32;
        while i < caracteres.len() && caracteres[i].is_ascii_digit() && i - debut < 3
          {
            valeur = valeur * 10 + caracteres[i].to_digit(10).unwrap();
            i += 1;
          }
        if i == debut || valeur > 255
          {
            return None;
          }
        if groupe < 3
          {
            if caracteres.get(i) != Some(&'.')
              {
                return None;
              }
            i += 1;
          }
      }
    if matches!(caracteres.get(i), Some(c) if c.is_ascii_digit() || *c == '.')
      {
        return None;
      }
    Some(i)
  }

fn masquer_mac(texte: &str) -> String
  {
    let caracteres: Vec<char> = texte.chars().collect();
    let mut sortie = String::with_capacity(texte.len());
    let mut i = 0;
    while i < caracteres.len()
      {
        if let Some(longueur) = essayer_mac(&caracteres[i..])
          {
            sortie.push_str("xx:xx:xx:xx:xx:xx");
            i += longueur;
          }
        else
          {
            sortie.push(caracteres[i]);
            i += 1;
          }
      }
    sortie
  }

fn essayer_mac(caracteres: &[char]) -> Option<usize>
  {
    let mut i = 0;
    for groupe in 0..6
      {
        let debut = i;
        while i < caracteres.len() && i - debut < 2 && caracteres[i].is_ascii_hexdigit()
          {
            i += 1;
          }
        if i - debut != 2
          {
            return None;
          }
        if groupe < 5
          {
            if caracteres.get(i) != Some(&':')
              {
                return None;
              }
            i += 1;
          }
      }
    Some(i)
  }

fn masquer_secrets(texte: &str) -> String
  {
    let minuscule = texte.to_ascii_lowercase();
    for mot in ["password=", "--password", "passwd=", "token=", "api_key=", "secret="]
      {
        if minuscule.contains(mot)
          {
            return "[secret masqué]".to_string();
          }
      }
    texte.to_string()
  }

#[cfg(test)]
mod tests
  {
    use super::*;

    #[test]
    fn une_ip_est_masquee()
      {
        assert_eq!(masquer("ping 192.168.1.20 ok"), "ping x.x.x.x ok");
      }

    #[test]
    fn un_secret_est_masque()
      {
        assert_eq!(masquer("curl --password=abc"), "[secret masqué]");
      }

    #[test]
    fn le_texte_ordinaire_est_intact()
      {
        assert_eq!(masquer("ls -la /tmp"), "ls -la /tmp");
      }

    #[test]
    fn un_drapeau_est_construit_depuis_le_code_pays()
      {
        assert_eq!(drapeau("fr"), "\u{1F1EB}\u{1F1F7}");
        assert_eq!(drapeau("??"), "");
      }

    #[test]
    fn les_debits_sont_lisibles()
      {
        assert_eq!(debit_lisible(512.0), "512 o/s");
        assert_eq!(debit_lisible(1536.0), "1.5 Ko/s");
      }
  }

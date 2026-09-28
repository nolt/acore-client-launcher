# Wydawanie aktualizacji

*[English version](releasing-updates.md)*

Strona administratora: gdzie leżą pliki na serwerze, jak opublikować aktualizację klienta i jak
wydać nowy launcher. Budowanie plików — zobacz [Budowanie](building.pl.md); formaty plików —
zobacz [Format manifestu](manifest-format.pl.md).

## Konfiguracja jednorazowa

Ustaw `MANIFEST_URL` w [`crates/app/src/config.rs`](../crates/app/src/config.rs) na pełny adres
`version.json` na swoim serwerze, np. `https://example.org/patch/version.json`, i zbuduj
launcher. Wszystko inne jest szukane względem niego:

- pliki gry — obok `version.json` albo w `baseUrl`, jeśli manifest go podaje;
- `launcher.json` — obok `version.json` (`LAUNCHER_INFO` w `config.rs`).

## Układ plików na serwerze

Wystarczy dowolny serwer WWW udostępniający pliki statyczne. Powinien obsługiwać zapytania `Range`
(Apache i nginx obsługują), żeby przerwane i wstrzymane pobieranie było wznawiane, a nie
zaczynane od nowa.

```
/srv/patch/                  ← dostępne jako https://example.org/patch/
├── version.json             ← manifest gry (MANIFEST_URL wskazuje tutaj)
├── launcher.json            ← informacja o self-update, obok version.json
├── launcher/
│   ├── acore-launcher       ← pliki launchera do self-update i do Twojej strony pobierania
│   └── acore-launcher.exe
├── Wow.exe                  ← klient dokładnie w takiej postaci, w jakiej mają go gracze
├── Data/…
└── …
```

### Apache

Jeśli na serwerze działa już strona (np.
[acore-simple-web](https://github.com/nolt/acore-simple-web)), trzymaj klienta poza folderem
strony i podepnij go aliasem, wewnątrz `<VirtualHost>` strony:

```apache
Alias /patch /srv/patch
<Directory /srv/patch>
    Require all granted
    Options -Indexes
    AllowOverride None
    # only files to download, never run PHP from here
    php_admin_flag engine off
</Directory>
```

```bash
apache2ctl configtest && systemctl reload apache2
chmod -R a+rX /srv/patch        # serwer WWW musi móc odczytać każdy plik
```

### nginx

```nginx
location /patch/ {
    alias /srv/patch/;
    autoindex off;
}
```

### Szybki serwer testowy

Do testu na własnym komputerze wystarczy wbudowany serwer Pythona — zobacz
[Budowanie → Test lokalny](building.pl.md#test-lokalny-przed-publikacją). Nie wznawia on
pobierania, więc nie używaj go dla graczy.

## Wydanie aktualizacji klienta

1. Trzymaj **lokalny folder z klientem dokładnie w takiej postaci, w jakiej mają go gracze** —
   osobno od folderu na serwerze i bez plików launchera.
2. Wprowadź tam zmiany (nowa łatka, tłumaczenie, `realmlist.wtf` …) i wygeneruj manifest:

   ```bash
   out/dist/acore-launcher-cli manifest /ścieżka/do/klienta
   # /ścieżka/do/klienta/version.json: version 2026.09.28, 3412 files, 17.1 GB
   ```

   | Opcja | Znaczenie |
   |---|---|
   | `--version V` | Wersja, którą gracze widzą jako „Zaktualizowano do V”. Domyślnie dzisiejsza data (UTC), np. `2026.09.28`. Tylko informacyjna — o tym, co pobrać, decydują hashe. |
   | `--base-url URL` | Skąd serwowane są pliki, względem manifestu. Domyślnie obok niego. |
   | `--output FILE` | Zapis manifestu gdzie indziej. Domyślnie `version.json` w folderze. |

   Ukryte pliki i foldery (nazwy zaczynające się od `.`) są pomijane i wypisywane, bo serwery WWW
   zwykle ich nie udostępniają.

3. **Najpierw wgraj pliki, a `version.json` na końcu**, żeby gracze nigdy nie dostali manifestu
   wskazującego pliki, których jeszcze nie ma. `rsync` zrobi to w dwóch krokach:

   ```bash
   rsync -av --exclude version.json /ścieżka/do/klienta/ host:/srv/patch/
   rsync -av /ścieżka/do/klienta/version.json host:/srv/patch/
   ```

Przy następnym uruchomieniu gracze pobiorą tylko pliki, których hash się zmienił. Pliki usunięte
z manifestu zostają na dyskach graczy — launcher nigdy nie usuwa plików gry.

> **Nie generuj manifestu na folderze serwera.** Manifest wymienia wszystko w folderze, więc pliki
> z `launcher/` stałyby się „plikami gry”, które pobiera każdy gracz, a launcher leżący w katalogu
> głównym dostałby polecenie nadpisania samego siebie.

### Tylko własne pliki

Launchera nie obchodzi, czy manifest wymienia całego klienta, czy kilka plików. Jeśli gracze mają
własnego klienta 3.3.5a, publikuj tylko to, co dodajesz — polską łatkę, `realmlist.wtf`, poprawki
— i poproś ich, żeby przy instalacji wskazali swój folder z WoW. Launcher pobierze wtedy megabajty
zamiast całego klienta.

## Wydanie nowego launchera

1. Podbij `version` w głównym [`Cargo.toml`](../Cargo.toml) (np. `0.1.0` → `0.2.0`) i uruchom
   `scripts/build.sh`.
2. Skopiuj nowe pliki na serwer, np. do `launcher/`, i wygeneruj `launcher.json` obok
   `version.json`:

   ```bash
   out/dist/acore-launcher-cli launcher 0.2.0 \
       --linux /srv/patch/launcher/acore-launcher \
       --windows /srv/patch/launcher/acore-launcher.exe \
       --output /srv/patch/launcher.json
   ```

   Narzędzie liczy hashe i zapisuje ścieżki względem `launcher.json` (`launcher/acore-launcher`).
   Jeden z systemów można pominąć.
3. Najpierw wgraj pliki, potem `launcher.json`.

Przy następnym uruchomieniu każdy launcher starszy niż opublikowana wersja pobierze nowy, podmieni
się i uruchomi ponownie; launcher z tą samą lub nowszą wersją nic nie robi. Bez `launcher.json`
na serwerze self-update po prostu nie działa.

## Co widzą gracze

- Launcher sprawdza nowego launchera, a potem pliki gry, **przy każdym uruchomieniu**. Nie ma
  usługi w tle i nic nie działa, gdy launcher jest zamknięty.
- W trakcie pobierania status pokazuje plik, postęp, prędkość i pozostały czas; gracz może
  wstrzymać i wznowić pobieranie.
- Po aktualizacji status mówi „Zaktualizowano do *wersja* — możesz grać.”

Więcej w [Przewodniku gracza](player-guide.pl.md).

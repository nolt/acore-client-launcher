# Format manifestu

*[English version](manifest-format.md)*

Opis plików, które launcher czyta z Twojego serwera i trzyma w folderze gry gracza. Pliki na
serwer zwykle generuje się przez `acore-launcher-cli`; zobacz
[Wydawanie aktualizacji](releasing-updates.pl.md).

## `version.json` — manifest gry

```json
{
  "version": "2026.09.28",
  "generatedAtUtc": "2026-09-28T12:00:00Z",
  "baseUrl": "",
  "files": [
    { "path": "Data/plPL/patch-plPL-4.MPQ", "hash": "<sha-256, małe litery hex>", "size": 163053568 },
    { "path": "Wow.exe", "hash": "<sha-256>", "size": 7704216 }
  ]
}
```

| Pole | Wymagane | Znaczenie |
|---|---|---|
| `version` | tak | Gracze widzą je jako „Zaktualizowano do *wersja*”. Dowolny tekst; o niczym nie decyduje. |
| `generatedAtUtc` | nie | Kiedy manifest wygenerowano. Informacyjne. |
| `baseUrl` | nie | Skąd serwowane są pliki, względem manifestu (albo pełny adres). Puste = obok `version.json`. |
| `files[].path` | tak | Ścieżka względem folderu gry, z `/`. Spacje i `#` są w porządku; w adresie zostaną zakodowane. |
| `files[].hash` | tak | SHA-256 pliku, 64 cyfry szesnastkowe. **Tylko hash decyduje, czy plik zostanie pobrany.** |
| `files[].size` | tak | Rozmiar w bajtach — szybkie wstępne sprawdzenie przed liczeniem hasha. |

Adres pobrania pliku to folder manifestu + `baseUrl` + `path`:
`https://example.org/patch/version.json` + `Data/plPL/patch-plPL-4.MPQ` →
`https://example.org/patch/Data/plPL/patch-plPL-4.MPQ`.

**Odrzucane manifesty.** Żeby pobieranie nie wyszło poza folder gry, launcher odrzuca cały
manifest („Serwer aktualizacji przysłał błędną listę plików”), jeśli ścieżka:

- jest bezwzględna, zawiera `..`, `.`, pusty człon (`a//b`), `\`, `:` albo znak NUL;
- różni się od innej ścieżki tylko wielkością liter (`Data/a.txt` i `data/A.txt` to na Windowsie
  i macOS ten sam plik);
- jest zarezerwowana dla launchera: `.launcher-state.json`, `launcher.local.json` albo cokolwiek
  kończącego się na `.part` (dowolna wielkość liter);

albo jeśli hash nie ma 64 cyfr szesnastkowych.

## `launcher.json` — self-update

```json
{
  "version": "0.2.0",
  "files": {
    "linux-x64":   { "path": "launcher/acore-launcher",     "hash": "<sha-256>", "size": 16479920 },
    "windows-x64": { "path": "launcher/acore-launcher.exe", "hash": "<sha-256>", "size": 13176320 }
  }
}
```

- Leży obok `version.json` (nazwa to `LAUNCHER_INFO` w `config.rs`).
- `path` jest względna do `launcher.json`.
- Launcher aktualizuje się tylko wtedy, gdy `version` jest **wyższa** niż jego własna, porównując
  liczba po liczbie (`0.10.0` > `0.9.3`; coś, co nie jest samymi liczbami, nigdy nie jest nowsze),
  i gdy jego system ma wpis.
- Brak pliku na serwerze (404) = brak self-update, launcher przechodzi do plików gry.

## `launcher.local.json` — ustawienia Wine gracza (Linux)

Opcjonalny, tworzy go gracz, w folderze gry. Launcher nigdy go nie tworzy ani nie pobiera,
a manifest nie może go zawierać.

```json
{ "winePrefix": "~/.wine-azerothcore", "wineCommand": "wine" }
```

| Pole | Domyślnie | Znaczenie |
|---|---|---|
| `winePrefix` | `WINEPREFIX` ze środowiska, a bez niego `~/.wine` | Prefiks Wine, w którym działa gra. `~/` oznacza folder domowy. |
| `wineCommand` | `wine` | Program Wine: `wine`, `wine64` albo pełna ścieżka. |

Inne pola są pomijane. Uszkodzony plik nie blokuje GRAJ: launcher wraca do ustawień domyślnych.

## Pliki w folderze gry

| Plik | Co to jest |
|---|---|
| `.launcher-state.json` | Rozmiar, czas modyfikacji i hash każdego sprawdzonego pliku, żeby przy następnym starcie nie liczyć hashy niezmienionych plików. Oznacza też folder jako zainstalowany — bez niego launcher pokazuje okno instalacji. Można go usunąć: następny start raz policzy wszystko od nowa. |
| `<plik>.part` | Niedokończone pobieranie. Następna próba je kontynuuje; staje się właściwym plikiem dopiero, gdy rozmiar i hash się zgadzają. |
| `.launcher-update` | Nowy launcher w trakcie self-update; usuwany po podmianie. |
| `launcher.local.json` | Ustawienia Wine gracza, zobacz wyżej. |
| `acore-launcher`, `acore-launcher.exe` | Sam launcher — mieszka w folderze gry. |

Skróty, poza folderem gry:

| System | Pliki |
|---|---|
| Linux | `~/.local/share/applications/<nazwa>-launcher.desktop`, `~/.local/share/icons/hicolor/256x256/apps/<nazwa>-launcher.png` i kopia pliku `.desktop` na pulpicie |
| Windows | `Start Menu\Programs\<app-name>.lnk`, `Desktop\<app-name>.lnk` |

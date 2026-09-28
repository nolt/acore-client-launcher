# Budowanie launchera

*[English version](building.md)*

Jak zbudować launcher, gdzie trafiają pliki i jak wypróbować go na lokalnym serwerze WWW przed
publikacją. Co zmienić przed buildem — zobacz [Wygląd](branding.pl.md); jak wgrać wynik na
serwer — zobacz [Wydawanie aktualizacji](releasing-updates.pl.md).

## Wymagania

- **Docker.** Wszystko buduje się w `docker/Dockerfile.build` (na bazie `rust:1-bookworm`,
  z MinGW do `.exe`). Na maszynie do budowania nie trzeba niczego więcej, także Rusta.
- Około **6 GB** wolnego miejsca na obraz i skompilowane zależności w `target/`.

Plik na Linuksa korzysta z glibc 2.36 (Debian 12), więc działa na nim i każdej nowszej
dystrybucji.

## `scripts/build.sh`

```bash
scripts/build.sh
```

Wypisuje jedną linię na etap, kropkę co 15 sekund w trakcie etapu i listę plików na końcu:

```
Building acore-launcher. The first build takes several minutes, later ones are quick.
[1/4] Preparing the build environment . done (23 s)
[2/4] Downloading dependencies  done (13 s)
[3/4] Compiling for Linux ................ done (4 min 13 s)
[4/4] Compiling for Windows ........... done (2 min 52 s)

Done in 7 min 41 s. Files in /ścieżka/do/acore-client-launcher/out/dist:
  acore-launcher              16M  launcher for Linux players
  acore-launcher.exe          13M  launcher for Windows players
  acore-launcher-cli         2.3M  manifest and launcher.json tool (Linux)
  acore-launcher-cli.exe     2.2M  manifest and launcher.json tool (Windows)
Full log: /ścieżka/do/acore-client-launcher/out/build.log
```

- **Pierwszy** build trwa kilka minut: buduje obraz i kompiluje wszystkie zależności dla obu
  systemów. Kolejne korzystają z gotowych i kompilują tylko to, co się zmieniło — zwykle w minutę
  lub dwie.
- Pełne wyjście kompilatora trafia do `out/build.log`. Gdy etap się nie powiedzie, skrypt pokazuje
  jego ostatnie 30 linii i przerywa.

## Co dostają gracze

| Plik w `out/dist/` | Dla kogo |
|---|---|
| `acore-launcher` | Gracze na Linuksie (x86-64) |
| `acore-launcher.exe` | Gracze na Windowsie 10/11 (x86-64) |
| `acore-launcher-cli`, `acore-launcher-cli.exe` | Ty: generuje `version.json` i `launcher.json`, uruchamia aktualizację z wiersza poleceń |

Gracze dostają tylko launcher na swój system i nic więcej. Licencje są w nim wbudowane (O programie
→ Licencje). Przy pierwszym uruchomieniu launcher sam się instaluje — zobacz
[Przewodnik gracza](player-guide.pl.md).

Nazwę plików ustawia `crates/app/Cargo.toml`; zobacz [Wygląd](branding.pl.md#nazwa-pliku).

## Test lokalny przed publikacją

Cały cykl — instalację, pobieranie, GRAJ — sprawdzisz na tymczasowym serwerze WWW, bez ruszania
prawdziwego serwera i bez przebudowy pod testowy adres.

```bash
# 1. Folder z klientem w takiej postaci, w jakiej mają go gracze; wygeneruj manifest.
out/dist/acore-launcher-cli manifest ~/test-host

# 2. Udostępnij go przez HTTP (do testu wystarczy wbudowany serwer Pythona).
cd ~/test-host && python3 -m http.server 8000

# 3. W drugim terminalu uruchom launcher z testowym adresem. Musi wskazywać version.json.
LAUNCHER_MANIFEST_URL=http://localhost:8000/version.json out/dist/acore-launcher
```

Launcher zapyta o folder instalacji, przeniesie się tam i pobierze pliki. Warto wiedzieć:

- Serwer Pythona nie obsługuje wznawiania (`Range`). Po **Pauza → Wznów** plik, który był
  w trakcie, zaczyna się od zera; Apache i nginx kontynuują od miejsca przerwania.
- Skróty tworzone przez launcher nie znają `LAUNCHER_MANIFEST_URL`; uruchomiony ze skrótu używa
  wbudowanego adresu. Na czas testów uruchamiaj go z terminala.
- Z `LAUNCHER_GAME_DIR=/jakiś/folder` pomija okno instalacji i używa tego folderu jako folderu gry.

### Zmienne środowiskowe do testów

| Zmienna | Działanie |
|---|---|
| `LAUNCHER_MANIFEST_URL` | Zastępuje `MANIFEST_URL` z `config.rs`. Pełny adres `version.json`. |
| `LAUNCHER_GAME_DIR` | Używa tego folderu gry zamiast folderu launchera i pomija instalację. |
| `MOCK=<stan>` | Pokazuje stały ekran bez sieci: `downloading`, `paused`, `ready`, `no-wine`, `error`, `install`, `install-error`, `tools`, `clear-cache`, `about`, `licenses`. |
| `SLINT_BACKEND=winit-software` | Wymusza renderowanie programowe (na maszynach bez OpenGL). |
| `LANG=pl_PL.UTF-8` | Język interfejsu na Linuksie (launcher idzie za językiem systemu). |

### Aktualizacja z wiersza poleceń

`acore-launcher-cli update` robi to samo co launcher, tylko bez okna, i przy błędzie wypisuje
dokładną przyczynę — najszybszy sposób na sprawdzenie serwera:

```bash
out/dist/acore-launcher-cli update http://localhost:8000/version.json ~/test-gra
out/dist/acore-launcher-cli update http://localhost:8000/version.json ~/test-gra --full-check
```

## Praca nad kodem

Wszystko działa w obrazie buildu:

```bash
docker build -t acore-launcher-build -f docker/Dockerfile.build docker
run() { docker run --rm -v "$PWD":/src -w /src -e CARGO_HOME=/src/target/cargo-home acore-launcher-build "$@"; }

run cargo test --workspace
run cargo clippy --workspace --all-targets
run cargo fmt --all
```

### Skrypty

| Skrypt | Co robi |
|---|---|
| `scripts/build.sh` | Pliki wydania na Linuksa i Windowsa w `out/dist/`. |
| `scripts/mockup-shots.sh` | Zrzuty wszystkich stanów `MOCK`: PNG w `docs/mockups/` i JPG używane w dokumentacji w `docs/assets/`. `UI_LANG=pl_PL scripts/mockup-shots.sh` robi polskie (`*.pl.jpg`). Uruchom oba po zmianie wyglądu. |
| `scripts/licenses.sh` | Generuje na nowo `THIRD-PARTY-LICENSES.md` przez `cargo-about`. Uruchom po dodaniu lub aktualizacji zależności — plik jest wkompilowany w launcher. |
| `scripts/wine-test.sh` | Instaluje `.exe` z `Downloads` pod Wine 10 w jednorazowym kontenerze; zrzuty i powstałe pliki trafiają do `out/wine-test/`. |

### Tłumaczenia

Zobacz [Wygląd → Język](branding.pl.md#język).

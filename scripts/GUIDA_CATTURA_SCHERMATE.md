# Guida alla cattura automatica delle schermate di SonarPad

Lo script `capture_sonarpad_screens.py` avvia SonarPad con un profilo temporaneo,
apre automaticamente editor, impostazioni e finestre secondarie, quindi crea:

- una cartella contenente le immagini PNG;
- `INDICE.md`, con l'elenco delle schermate;
- `PANORAMICA.png`, con tutte le miniature;
- un archivio ZIP pronto da inviare a GPT.

## Requisiti

- Windows;
- Python 3;
- Pillow installato per Python;
- una copia compilata di `sonarpad.exe`.

Per verificare Python e Pillow:

```powershell
python --version
python -c "from PIL import ImageGrab; print('Pillow installato')"
```

Se Pillow non è installato:

```powershell
python -m pip install Pillow
```

I comandi seguenti presumono che PowerShell sia aperto nella cartella principale
del progetto SonarPad.

## Compilare SonarPad con la Route Key

La Route Key è incorporata nell'eseguibile durante la compilazione. Serve per
Cinema, TV, Percorsi e le altre API protette che usano
`SONARPAD_ROUTE_CLIENT_TOKEN`.

Per evitare che la chiave venga scritta nella cronologia di PowerShell:

```powershell
$env:SONARPAD_ROUTE_CLIENT_TOKEN = Read-Host "Inserisci la Route Key" -MaskInput
cargo build --release
Remove-Item Env:\SONARPAD_ROUTE_CLIENT_TOKEN
```

L'eseguibile risultante si trova normalmente qui:

```text
target\release\sonarpad.exe
```

La variabile deve essere presente mentre viene eseguito `cargo build`. Rimuoverla
dopo la compilazione non elimina la chiave dall'eseguibile già prodotto.

Non scrivere la Route Key direttamente nel codice sorgente, nella guida o in un
file che potrebbe essere aggiunto a Git.

## Cattura consigliata

Per usare la build `release` e il codice Rai/SonarPad già salvato nelle
impostazioni locali:

```powershell
python scripts\capture_sonarpad_screens.py `
  --exe target\release\sonarpad.exe `
  --use-saved-rai-code
```

Lo script copia nel profilo temporaneo soltanto il codice Rai cifrato. Non copia
chiavi API, cronologia, documenti recenti o percorsi personali.

I risultati predefiniti vengono creati qui:

```text
artifacts\sonarpad-schermate\
artifacts\sonarpad-schermate.zip
```

## Usare un sonarpad.exe in un'altra posizione

Passare il percorso completo con `--exe`. Se contiene spazi, racchiuderlo tra
virgolette.

Esempio su un altro disco:

```powershell
python scripts\capture_sonarpad_screens.py `
  --exe "D:\Programmi\SonarPad\sonarpad.exe" `
  --use-saved-rai-code
```

Esempio nella cartella Download:

```powershell
python scripts\capture_sonarpad_screens.py `
  --exe "$env:USERPROFILE\Downloads\SonarPad\sonarpad.exe" `
  --use-saved-rai-code
```

Il file indicato deve essere una build che contiene la Route Key se si vogliono
acquisire anche Cinema e gli altri servizi protetti.

## Usare la posizione predefinita dello script

Senza `--exe`, lo script cerca automaticamente:

```text
target\debug\sonarpad.exe
```

Comando minimo:

```powershell
python scripts\capture_sonarpad_screens.py
```

Questa modalità non utilizza il codice Rai salvato e la build `debug` potrebbe
non contenere la Route Key.

## Scegliere cartella e ZIP di destinazione

È possibile indicare una cartella dedicata per le immagini e un percorso diverso
per lo ZIP:

```powershell
python scripts\capture_sonarpad_screens.py `
  --exe "D:\Programmi\SonarPad\sonarpad.exe" `
  --use-saved-rai-code `
  --output "D:\Catture\sonarpad-schermate" `
  --zip "D:\Catture\sonarpad-schermate.zip"
```

**Attenzione:** a ogni esecuzione, la cartella passata a `--output` viene
eliminata completamente e ricreata. Usare sempre una cartella dedicata alle
catture; non indicare una cartella contenente documenti o altri file importanti.
Anche il file indicato con `--zip` viene sostituito.

## Rigenerare le schermate

Non occorre cancellare manualmente i risultati precedenti. Lo script ricrea da
zero la cartella di output e sostituisce lo ZIP al termine della cattura:

```powershell
python scripts\capture_sonarpad_screens.py `
  --exe target\release\sonarpad.exe `
  --use-saved-rai-code
```

## Significato delle opzioni

| Opzione | Descrizione |
|---|---|
| `--exe PERCORSO` | Indica il `sonarpad.exe` da acquisire. |
| `--use-saved-rai-code` | Usa soltanto il codice Rai cifrato delle impostazioni correnti. |
| `--output CARTELLA` | Sceglie la cartella dedicata alle immagini. |
| `--zip FILE` | Sceglie il percorso dell'archivio ZIP finale. |

Per vedere l'elenco aggiornato delle opzioni:

```powershell
python scripts\capture_sonarpad_screens.py --help
```

## Problemi comuni

### Cinema restituisce HTTP 401 o 403

L'eseguibile probabilmente non contiene una Route Key valida. Ripetere la
compilazione impostando `SONARPAD_ROUTE_CLIENT_TOKEN` prima di `cargo build` e
usare esplicitamente il nuovo eseguibile con `--exe`.

### Compare la richiesta del codice Rai o SonarPad

Usare `--use-saved-rai-code` e verificare che il codice sia già stato salvato
nelle impostazioni della copia normalmente utilizzata di SonarPad.

### Lo script segnala che l'eseguibile non esiste

Controllare il percorso passato a `--exe`. Per visualizzare il percorso completo:

```powershell
Resolve-Path target\release\sonarpad.exe
```

### Una finestra online è vuota o mostra un errore

Alcune schermate dipendono dalla connessione Internet e dai relativi servizi.
Ripetere la cattura quando il servizio è raggiungibile.

### Interrompere una cattura

Premere `Ctrl+C` nella finestra PowerShell. Lo script chiude il processo SonarPad
avviato per la cattura e rimuove il profilo temporaneo.

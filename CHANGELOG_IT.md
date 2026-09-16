# Changelog

Versione 0.9.10 – 2026-09-16

Audiodescrizioni
1. Aggiunta la nuova funzione Sonarpad Audiodescrizioni, per ascoltare i film audiodescritti con Sonarpad.

2. Migliorata l’apertura delle audiodescrizioni Rai: non vengono più scaricate preventivamente, ma vengono aperte immediatamente. Se l’utente vorrà scaricarle potrà sempre farlo su sua richiesta.

3. Migliorate le audiodescrizioni sostituendo il carattere `_` (trattino basso) con uno spazio, per evitare che nelle audiodescrizioni la sintesi vocale pronunci quel carattere.

Versione 0.9.9 – 2026-09-15

Audiodescrizione con IA
1. Aggiunto un fallback isolato per i video sorgente privi di traccia audio, senza modificare la pipeline già funzionante. Sonarpad tenta sempre prima l’esportazione normale; soltanto se FFmpeg segnala la mancanza dello stream audio e Sonarpad conferma che il file non contiene alcuna traccia audio, viene creata una sorgente silenziosa della stessa durata sulla quale vengono mixate le descrizioni sintetizzate. Se si esporta sul video originale, viene poi riutilizzato il normale percorso già esistente di mux video+audio. I video che contengono già audio continuano a seguire esattamente il percorso normale.

2. Esteso il fallback strettamente isolato per l’assenza di descrizioni senza modificare la normale pipeline dell’audiodescrizione. Sonarpad esegue sempre per prima l’analisi esistente; soltanto quando termina senza descrizioni utilizzabili propone il secondo tentativo in modalità Breve, che usa la stessa pipeline mantenendo attivi tutti i vincoli sui silenzi e il divieto di sovrapporsi ai dialoghi. Se anche questo secondo tentativo non riesce a inserire una descrizione, Sonarpad chiede ora un consenso esplicito per un ultimo fallback. Solo dopo aver scelto Sì, riutilizza le descrizioni brevi già generate da Gemini e salvate nel checkpoint del secondo tentativo e le mixa ai rispettivi tempi visivi con ducking, consentendo brevi sovrapposizioni ai dialoghi. Pyannote, il bridge Gemini, le regole normali di allineamento, la schedulazione TTS e i primi due percorsi di analisi non vengono modificati. Se l’utente rifiuta, oppure non esistono descrizioni generate da riutilizzare, Sonarpad termina correttamente con un messaggio localizzato.

3. Perfezionato il fallback isolato per l’assenza di descrizioni senza modificare la normale pipeline dell’audiodescrizione. Se la verbosità scelta inizialmente dall’utente è già Breve e l’analisi termina senza descrizioni utilizzabili, Sonarpad ora salta la seconda analisi Gemini ridondante e, quando sono disponibili descrizioni generate da riutilizzare, chiede direttamente se usare il fallback finale con sovrapposizione ai dialoghi. Se invece la verbosità iniziale è Normale o Dettagliata, il tentativo Breve viene mantenuto anche quando le pause disponibili sono molto corte: in quel caso serve anche a generare una descrizione più breve da usare nell’eventuale fallback finale, riducendo il tempo in cui la narrazione copre il dialogo. Pyannote, il bridge Gemini e le normali regole sui silenzi e sull’allineamento restano invariati.

Registrazione podcast
1. Aggiunto un fallback conservativo per la posizione del dispositivo del microfono con alcuni driver WASAPI problematici, senza modificare la registrazione sui sistemi che già funzionano. Il fallback si attiva soltanto dopo mismatch persistenti della posizione del dispositivo: 24 consecutivi oppure almeno l’80% su 64 pacchetti puliti. I veri `DATA_DISCONTINUITY` di WASAPI e gli errori di timestamp restano sempre validi e la cattura dell’audio di sistema non viene modificata.

YouTube e streaming
1. Corretti i download YouTube falliti avviati dal menu contestuale dei risultati. Gli errori noti relativi ai video riservati ai membri del canale vengono ora trasformati nel messaggio localizzato di Sonarpad invece di mostrare il testo grezzo in inglese di yt-dlp. Dopo aver chiuso l’errore, Sonarpad esegue inoltre un ripristino differito del focus al termine del menu/modale nativo, tornando in modo affidabile alla lista dei risultati con tastiera attiva. La pipeline dei download che riescono normalmente resta invariata.

2. Allineato il filtro preventivo a SonarTube mobile: nei risultati di ricerca e nella navigazione di canali/playlist vengono ora nascosti i video per i quali YouTube/yt-dlp non restituisce alcun dato sulle visualizzazioni, mentre canali e playlist restano visibili e un vero video con 0 visualizzazioni viene mantenuto. La gestione localizzata dell’errore per i contenuti riservati ai membri resta attiva come secondo fallback. Il download multiplo delle playlist non viene modificato.

Versione 0.9.8 – 2026-09-12

Audiodescrizione con IA
1. Rafforzata in modo conservativo l’esportazione delle audiodescrizioni con tracce multicanale, senza modificare la pipeline dei file che già funzionano. Sonarpad continua sempre con il layout di canali originale; soltanto se il WAV intermedio multicanale fallisce per un frame/campione non allineato al numero di canali, viene ripetuta solo quell’esportazione con un fallback stereo. File mono, stereo e multicanale che vengono già esportati correttamente seguono esattamente il percorso normale.

Tema scuro e accessibilità
1. Corretto il tema scuro che interferiva con le finestre native di Windows e con i messaggi di conferma. Sonarpad non applica più il proprio subclass del tema scuro agli alberi di dialogo di sistema `#32770`: le finestre Apri/Salva, l’esportazione diagnostica e i MessageBox restano gestiti da Windows e possono ricevere correttamente il focus da tastiera e NVDA. Corretto inoltre un problema interno con l’estensione ZIP predefinita della finestra Salva della diagnostica.

Versione 0.9.7 – 2026-09-11

Audiodescrizione con IA
1. Aggiunta l’opzione facoltativa “Crea l’audiodescrizione sul file video originale”. Quando è attiva, Sonarpad ora preferisce MP4: copia il flusso video originale senza ricodifica e inserisce la traccia audio audiodescritta. Se FFmpeg segnala un’incompatibilità di contenitore o di scrittura dei pacchetti durante la creazione dell’MP4, Sonarpad riprova automaticamente la stessa esportazione in MKV, sempre senza ricodificare il video. È inoltre possibile scegliere esplicitamente MKV. In questa modalità le pause estese vengono ignorate per mantenere sincronizzati audio e video.

2. Migliorata la rianalisi dei segmenti nei progetti audiodescrittivi salvati. La finestra usa ora l’accesso AI già configurato nella finestra principale Crea audiodescrizione, quindi non duplica più chiave API, credito Sonarpad AI e scelta del modello. Un segmento rianalizzato conserva ora l’intero gruppo di descrizioni del progetto appartenenti a quel chunk di analisi, invece della sola prima/più vicina: tutte le descrizioni del gruppo vengono marcate come rianalizzate e “Applica segmento e riesporta MP3” applica l’intero gruppo in una sola operazione. Le anteprime delle pause estese vengono sintetizzate direttamente invece di cercare il punto nell’MP3 esportato, evitando anteprime che partono con l’audio del film o tagliano la descrizione.

3. Migliorato il fallback conservativo per i media inviati a Gemini senza modificare la pipeline già funzionante. L’errore HTTP 400 `INVALID_ARGUMENT` continua ad attivare i chunk MKV più piccoli (circa 15 MB) e poi i chunk MP4 compatibili. Inoltre, se Gemini accetta un chunk ma la sua elaborazione termina in `FAILED`, Sonarpad ricarica una volta esattamente lo stesso chunk e solo dopo passa al fallback MP4; i retry per il server Code 13 sono limitati a tre prima dello stesso fallback, evitando attese infinite. Errori di rete, quota, autenticazione o sintesi non attivano questo percorso.

4. Corretta la persistenza delle credenziali AI quando si cambia modalità di accesso. La chiave API Gemini personale e il codice Sonarpad AI vengono ora salvati in modo indipendente: passando da Chiave API personale a Sonarpad AI, o viceversa, la credenziale inattiva non viene più cancellata. La chiave Gemini viene inoltre salvata quando il relativo campo perde il focus.

5. Aggiunto un vero pulsante Annulla per “Rianalizza segmento” e “Applica segmento e riesporta MP3”. La barra di avanzamento resta attiva, mentre Annulla ora interrompe la preparazione FFmpeg del segmento, il worker AI, la verifica TTS e l’esportazione finale non appena la fase in corso può fermarsi in sicurezza. L’applicazione di un segmento rianalizzato è ora transazionale: progetto e file multimediale esistenti vengono sostituiti solo dopo una riesportazione riuscita; se si annulla, i file precedenti restano invariati e la proposta rianalizzata rimane disponibile per un nuovo tentativo.

6. Corretta la rianalisi dei segmenti per le descrizioni che si trovano oltre il primo chunk Gemini. Il segmento isolato viene ora inviato al worker esistente con una timeline locale che parte da zero e poi ricondotto internamente alla posizione assoluta del progetto, evitando `Invalid prepared chunk timeline at chunk 1` senza modificare la pipeline normale delle audiodescrizioni.

7. Rielaborata la rianalisi dei segmenti dei progetti salvati per usare gli stessi controlli di sicurezza della creazione completa dell’audiodescrizione. Il chunk selezionato esegue ora la stessa estrazione audio mono a 16 kHz e la stessa analisi dialoghi/silenzi con Pyannote, le stesse regole temporali e gli stessi fallback Gemini, la stessa sintesi TTS reale con la voce del progetto e lo stesso scheduler finale per il posizionamento sicuro e le pause estese. Quando il segmento viene applicato, Sonarpad conserva i nuovi tempi assoluti già verificati e aggiorna la protezione Pyannote di quel chunk; è stata rimossa la precedente soluzione speciale dell’anteprima per i testi rianalizzati troppo lunghi.

8. Corretta la sostituzione strutturale del segmento dopo la rianalisi completa. Sonarpad non richiede più che il nuovo segmento verificato contenga esattamente lo stesso numero di descrizioni del progetto salvato: le vecchie descrizioni di quel chunk vengono sostituite dall’intero risultato sicuro della pipeline completa, quindi un segmento con 10 descrizioni può correttamente diventare 9 (o 11). L’editor del progetto mostra subito il nuovo numero e i nuovi tempi per l’anteprima, mentre progetto e file multimediale salvati restano invariati finché “Applica segmento e riesporta MP3” non termina con successo.

9. Rielaborata la rianalisi dei segmenti affinché il chunk fisico selezionato venga trattato come un vero mini-film autonomo. Sonarpad passa ora quel mini-film direttamente alla stessa identica funzione `create_audio_description` usata dalla creazione completa normale, così video e traccia audio condividono la stessa timeline locale e vengono eseguiti senza una pipeline separata per la rianalisi i normali controlli Pyannote, Gemini, TTS reale, scheduler e sicurezza. Solo al termine della pipeline normale i tempi locali risultanti vengono riportati alla posizione originale del film.

10. Corretta la sincronizzazione temporale della rianalisi quando il mini-film analizzato viene riportato sul film originale. Sonarpad applica ora la stessa piccola riconciliazione della durata dei chunk usata dall’analisi completa anche alle descrizioni, ai timestamp Visual Evidence e agli intervalli di dialogo protetti, evitando che uno scarto progressivo di pochi decimi di secondo verso la fine del chunk sposti la voce sopra il parlato.

Modifica testo
1. Migliorato “Unisci righe spezzate” in Modifica > Testo. Senza selezione elabora l’intero documento; con una selezione interviene solo sulle righe selezionate. Ora ricostruisce veri paragrafi unendo tutte le righe consecutive anche quando una frase termina con punteggiatura; le righe vuote restano separatori di paragrafo e gli elenchi numerati o puntati vengono mantenuti.

Versione 0.9.6 – 2026-09-09

1. Corretti i blocchi di alcune voci SAPI4 con lo spostamento del cursore attivo. Il bridge ora attende il completamento effettivo della sintesi, mantenendo attivo lo spostamento del cursore.

2. Corretti la sincronizzazione tra microfono e audio di sistema e i tic causati dagli arrotondamenti dei timestamp in Registra podcast. Le correzioni valgono anche salvando le sorgenti separatamente.

3. Isolata la sintesi SAPI5 delle audiodescrizioni in processi separati. In caso di errore, la sintesi viene riprovata con parallelismo ridotto conservando le descrizioni riuscite; il limite funzionante viene memorizzato per la voce.

Versione 0.9.5 – 2026-09-07

Audiodescrizione con IA
1. Migliorata la resilienza quando Gemini restituisce temporaneamente `MALFORMED_RESPONSE` senza contenuto utilizzabile. Sonarpad ora ritenta esattamente lo stesso chunk fino a tre volte, con una breve attesa tra i tentativi, invece di interrompere subito l’intera audiodescrizione. Le normali risposte di Gemini, i blocchi di sicurezza, gli errori di limite token e il comportamento dei checkpoint restano invariati.

2. Corretto un altro raro caso di `invalid Gemini chunk timeline` con alcuni video i cui chunk preparati da FFmpeg riportano un piccolo scostamento cumulativo della durata. Sonarpad mantiene invariato il percorso normale quando la timeline è valida e usa un fallback conservativo di riallineamento solo quando la timeline normale fallirebbe e lo scostamento totale è piccolo; differenze grandi o sospette continuano a produrre un errore.

3. Aggiunto il servizio opzionale Sonarpad AI per le audiodescrizioni con IA. Gli utenti possono continuare a usare la propria chiave API Gemini oppure scegliere il servizio Sonarpad e inserire un codice Sonarpad personale. Il codice viene protetto con Windows DPAPI. In modalità servizio, i chunk video preparati vengono caricati direttamente dal PC dell’utente a Google tramite un URL temporaneo di upload Google; sonarpad.com non riceve né conserva i byte del video. Il backend autorizza soltanto l’accesso, impone modello Gemini/Standard e limiti di utilizzo, contabilizza il consumo e restituisce la risposta di Gemini.

4. Migliorata la modifica dei progetti audiodescrittivi salvati. Ora è possibile cambiare più descrizioni una dopo l’altra senza dover applicare ogni modifica immediatamente. Le modifiche restano in memoria; premendo “Applica testo” Sonarpad verifica ogni descrizione modificata rispetto al relativo intervallo salvato e poi salva insieme tutte le modifiche valide. Se una descrizione non entra nel proprio range, il focus torna su quella descrizione senza perdere le altre modifiche ancora da applicare.

5. Aggiunto “Regola voce” immediatamente prima di “Crea audiodescrizione”. La nuova finestra permette di scegliere motore, voce, velocità e volume e include “Testa voce”. Premendo OK il focus torna esattamente su “Regola voce” e i valori scelti vengono applicati all’audiodescrizione che verrà creata. Se la finestra non viene mai usata, la sintesi resta esattamente come prima.


6. Ampliati i controlli della voce nei progetti audiodescrittivi salvati. In Modifica progetto, oltre a motore e voce, sono ora disponibili anche velocità, volume e “Testa voce”. Applicando i parametri voce, Sonarpad ricontrolla tutte le descrizioni esistenti con i nuovi parametri di sintesi e ricostruisce l’MP3 solo se continuano tutte a entrare nei rispettivi intervalli salvati. Gli intervalli senza dialogo, i tempi di Visual Evidence, le descrizioni Gemini e l’analisi temporale del progetto vengono riutilizzati senza eseguire nuovamente l’analisi IA.

Versione 0.9.4 – 2026-09-04

Audiodescrizione con IA
1. Corretto un problema con i video Matroska/WebM che contengono un timestamp iniziale interno insolitamente elevato e che potevano interrompere la generazione dell’audiodescrizione con l’errore “invalid Gemini chunk timeline”. Sonarpad ora normalizza la durata del file sorgente e dei chunk Gemini solo quando rileva questo schema anomalo di timestamp, lasciando invariati i video normali e il comportamento dell’audiodescrizione già funzionante.
2. Migliorato il ducking dell’audiodescrizione per ottenere un mix più naturale. L’audio originale ora si abbassa in modo morbido prima dell’inizio della voce e torna gradualmente al livello normale dopo la descrizione; le descrizioni molto ravvicinate mantengono inoltre un livello di fondo stabile, evitando continui abbassamenti e rialzi.
3. Corretto un problema che poteva far fallire l’esportazione finale dell’audiodescrizione con alcuni file AVI e altre sorgenti con audio 5.1/multicanale, quando la decodifica terminava con un frame audio interlacciato incompleto. Sonarpad ora completa in modo sicuro solo l’ultimo frame parziale con i campioni silenziosi strettamente necessari; i frame già completi e i normali file mono, stereo o multicanale restano invariati.


Versione 0.9.3 – 2026-09-03

Voci SAPI5
1. Corretto un problema per cui alcune voci SAPI5 locali potevano non parlare con lo spostamento del cursore attivo, durante la lettura multivoce o nella creazione di audiolibri/MP3. Sonarpad usa ora un percorso di sintesi SAPI5 su file affidabile in Windows e che può ancora essere annullato durante la sintesi, mentre la normale lettura diretta rimane invariata.
2. Corretta la posizione del cursore durante la lettura multivoce dei dialoghi. I tag voce inseriti automaticamente da Sonarpad per i dialoghi vengono ora considerati solo metadati di riproduzione e non più caratteri presenti nell’editor, evitando che dopo F4 o F6 il cursore salti in avanti rispetto al testo reale. La lettura a voce singola e i tag <voice> scritti esplicitamente nei documenti restano invariati.

Audiodescrizione con IA
1. Aggiunta la casella “Mostra chiave API” subito dopo il campo della chiave API Gemini. È disattivata per impostazione predefinita; quando viene attivata mostra temporaneamente la chiave completa, così è possibile verificare che sia stata incollata per intero. Riaprendo la finestra, la chiave torna sempre nascosta.

Podcast e Wikipedia
1. Dopo il salvataggio di una registrazione podcast, Sonarpad ora chiede se aprire la cartella che contiene il file salvato, come già avviene dopo il salvataggio dei media YouTube/streaming.
2. Quando si importa un altro articolo di Wikipedia in un editor che contiene già del testo, il nuovo articolo viene ora aggiunto in fondo invece che inserito in cima. Il cursore viene posizionato all’inizio del nuovo articolo appena importato.


Versione 0.9.2 – 2026-09-02

Audiodescrizione con IA
1. Corretto un problema che poteva far fallire l’audiodescrizione durante l’esportazione finale in MP3 con video contenenti audio multicanale, ad esempio 5.1. Sonarpad ora converte automaticamente l’audio multicanale in stereo solo quando necessario per la codifica MP3, senza modificare le esportazioni mono o stereo.
2. Quando si avvia Crea audiodescrizione con IA con un video che contiene più tracce audio, Sonarpad ora chiede quale traccia utilizzare prima di iniziare. La casella combinata accessibile si può cambiare con le frecce; OK avvia l’audiodescrizione usando la traccia scelta, mentre Annulla chiude la finestra dell’audiodescrizione e riporta il focus all’editor di Sonarpad.

YouTube e streaming
1. Corretto un problema per cui, avviando Crea audiodescrizione con IA da un video presente nella pagina 2 o successive di una playlist o di un canale YouTube, poteva riaprirsi la finestra di selezione YouTube sottraendo il focus alla finestra dell’audiodescrizione. Sonarpad ora chiude correttamente il selettore senza tornare alle pagine precedenti.
Versione 0.9.1 – 2026-09-01

Download YouTube
• Corretto un problema per cui le finestre di avanzamento dei download YouTube/streaming potevano tornare ripetutamente in primo piano dopo il passaggio a un'altra applicazione con Alt+Tab. I download ora continuano in background senza rubare il focus.
• Migliorata l'accessibilità dell'avanzamento dei download. Tornando alla finestra di progresso, gli screen reader possono leggere lo stato corrente e la percentuale. Nelle playlist Sonarpad indica anche il numero dell'elemento corrente, il totale degli elementi e il titolo.
• Corretto un falso rilevamento di blocco del watchdog durante download e conversioni lunghi quando la finestra di avanzamento continuava a rispondere.
• Aggiunta una casella combinata Formato nel download delle playlist. Dall’elenco dei video, premendo Tab è possibile scegliere MP4, MP3, M4A, OPUS, OGG, WAV o FLAC prima di avviare il download multiplo.
• Riorganizzato il salvataggio dei media in streaming. Formato e qualità vengono ora scelti al momento del salvataggio invece che nella finestra iniziale di ricerca streaming. “Salva media” apre un’unica finestra con Formato e Qualità e, nelle playlist, sono disponibili entrambe le caselle combinate.

Audiodescrizione con IA
• Corretto un problema che poteva impedire l’avvio dell’audiodescrizione con IA con alcuni video MKV. Sonarpad ora gestisce in modo più affidabile i video con timestamp irregolari o mancanti.

Versione 0.9.0 – 2026-08-31

Audiodescrizione con IA — nuova funzione principale
• Aggiunto in Strumenti > Multimedia il modulo “Crea audiodescrizione con IA”. Sonarpad analizza l’audio per individuare gli spazi liberi dai dialoghi, genera le descrizioni con Gemini e usa i motori vocali già presenti nel programma, evitando di parlare sopra le battute.
• Migliorata la sincronizzazione tra ciò che accade nel video e le descrizioni, con controlli automatici sui tempi generati da Gemini.
• “Attiva pause estese” è deselezionata per impostazione predefinita. Può essere attivata nei contenuti con molti dialoghi o poco spazio disponibile per permettere l’inserimento di descrizioni altrimenti troppo lunghe.
• È possibile provare a riconoscere i personaggi e usare i loro nomi. I cataloghi dei personaggi possono essere mantenuti tra gli episodi di una serie per migliorare la continuità.
• È possibile salvare il progetto, modificare in seguito le descrizioni e riesportare senza dover rigenerare tutto con Gemini.
• Se il lavoro viene interrotto, Sonarpad conserva i progressi e permette di continuare l’audiodescrizione. In caso di quota Gemini esaurita è possibile attendere, cambiare modello oppure interrompere senza perdere il lavoro già completato.
• La finestra permette di scegliere lingua, livello di dettaglio, modello Gemini, motore e voce e ricorda le preferenze utilizzate.
• Il modulo è disponibile nelle 17 lingue di Sonarpad. Durante la generazione l’interfaccia mostra soltanto avanzamento, stato corrente e pulsante Annulla; al termine l’MP3 può essere aperto direttamente nel player interno.
• “Crea audiodescrizione con IA” è disponibile anche per i video aperti tramite streaming e per i contenuti on demand compatibili di RaiPlay e La7 Play. Migliorata inoltre la gestione del focus quando la funzione viene aperta da questi menu o al termine di una registrazione TV.

La7 Play e multimedia
• Aggiunta La7 Play in Strumenti > Multimedia, con ricerca dei programmi, dirette La7 e La7 Cinema e riproduzione dei contenuti on demand non protetti nel player interno.
• Sui contenuti La7 scaricabili il menu contestuale propone ora Salva media, Trascrivi e Crea audiodescrizione con IA. Salva media permette di scegliere tra MP3 e MP4.
• Migliorata la gestione del focus nelle azioni contestuali di YouTube/streaming, RaiPlay e La7 Play, in modo che finestre di download, trascrizione e audiodescrizione restino correttamente in primo piano.
• Nel modulo Rai audiodescrizioni, Salva media mostra ora una finestra di avanzamento con possibilità di annullare e Trascrivi mantiene visibile la finestra di progresso fino al completamento.
• Aggiunto il tasto Canc per eliminare registrazioni TV e radio, con richiesta di conferma.

E-book e documenti
• Aggiunta l’importazione dei Kindle senza DRM nei formati MOBI, AZW e AZW3, con testo e capitoli disponibili nell’editor e nell’indice.
• Aggiunto il supporto DAISY 2.02 e DAISY 3. Gli audiolibri DAISY usano il player interno di Sonarpad e rispettano la navigazione e i limiti dei capitoli.
• Kindle e DAISY vengono importati senza sovrascrivere il file originale; i Kindle protetti da DRM vengono rifiutati esplicitamente.
• Corretto “Salva con nome” per gli EPUB: scegliendo TXT o un altro formato viene ora usata l’estensione selezionata e l’EPUB originale resta associato al documento aperto.

RSS e articoli
• Aggiunta la selezione multipla degli articoli RSS per eliminarne più di uno in un’unica operazione.
• Gli RSS supportano ora vere cartelle, mantenute anche durante importazione ed esportazione OPML, comprese le cartelle vuote.
• I feed possono essere riordinati all’interno della cartella corrente con i comandi Sposta su, Sposta giù, Sposta in cima, Sposta in fondo e Sposta alla posizione.

Accessibilità, guide e interfaccia
• La Guida TV mostra ora i programmi in una casella combinata selezionabile e permette di visualizzare la trama del programma scelto.
• Le guide di Sonarpad sono state riorganizzate con un indice ed è stata aggiunta una guida completa alla funzione Audiodescrizione con IA.
• Corretto un problema della traduzione tedesca che poteva impedire la comparsa delle finestre Apri, Salva con nome e di altre finestre di selezione file.

Voci e lingue
• Il catalogo delle voci Google TTS scaricabili passa da 104 a 156 pacchetti e da 53 a 81 varianti linguistiche.
• Aggiunti nuovi pacchetti Google TTS e i nomi localizzati di altre lingue in tutte le lingue dell’interfaccia.

Versione 0.8.4 – 2026-07-24

Modifica dei documenti EPUB
• Sonarpad è ora in grado non solo di aprire i documenti EPUB, ma anche di modificarli e salvarli nuovamente in formato EPUB mantenendo la formattazione originale, l’indice, le note a piè di pagina, le immagini, i fogli di stile, i metadati e i collegamenti interni.
• In “Salva con nome” il formato EPUB è disponibile per i documenti aperti da un EPUB. Il salvataggio aggiorna soltanto il testo modificato e conserva intatta la struttura del libro.

Registrazioni con audiodescrizione
• Corrette le registrazioni con traccia audiodescritta. Sonarpad riesce ora a registrare correttamente il video originale insieme alla traccia audio dei canali con audiodescrizione.

Affidabilità degli audiolibri
• Corretto un problema intermittente per cui, dopo cinque tentativi Google TTS falliti, un’unità di sintesi veniva eliminata silenziosamente e nell’audiolibro finale poteva mancare una parte del testo.
• Le unità Google vengono ora ritentate finché riescono oppure finché l’utente annulla. L’avvio dei processi viene scaglionato per ridurre i conflitti temporanei con Chrome e con i file; Sonarpad interrompe inoltre la creazione invece di salvare un audiolibro a cui manca un segmento.
• Anche gli audiolibri Edge ora ritentano senza limite gli errori temporanei di rete, WebSocket, timeout, limitazione del servizio e audio non valido, fino al successo o all’annullamento dell’utente, comprese le voci miste e la divisione per durata. SAPI4 e SAPI5 mantengono tentativi adattivi e finiti; se un segmento continua a fallire, Sonarpad interrompe l’operazione senza salvare un audiolibro incompleto.

Navigazione delle biblioteche digitali
• I risultati di LibriVox, Internet Archive e Project Gutenberg usano ora una navigazione a pagine come YouTube: “Vai ai risultati precedenti” compare all’inizio dell’elenco e “Vai ai risultati successivi” alla fine.
• Corretto il passaggio del focus in LibriVox: aprendo un libro o un capitolo, NVDA non viene più spostato nell’editor principale prima dell’apertura dell’elenco successivo o del lettore.
• Aggiunta una protezione del focus durante le ricerche e il caricamento dei libri LibriVox: una finestra di caricamento localizzata rimane in primo piano per tutta la richiesta, impedendo al focus di NVDA di passare al Prompt dei comandi, a Windows Terminal o a un’altra applicazione.

Download delle playlist YouTube
• Aggiunto alle playlist YouTube un comando accessibile di selezione multipla, che permette di scegliere quali video scaricare senza modificare il comando “Salva media” relativo all’elemento attualmente in riproduzione.
• Gli elementi selezionati vengono scaricati uno alla volta usando il formato e la qualità scelti all’apertura della playlist, ricevono nomi numerati che mantengono l’ordine originale e vengono salvati in una cartella dedicata all’interno della cartella Media configurata.
• La finestra comprende “Seleziona tutto” e “Deseleziona tutto”, annuncia quanti elementi sono selezionati, consente di annullare conservando i file già completati e segnala chiaramente gli elementi che non è stato possibile scaricare.
• Gli elementi della playlist sono ora vere caselle di controllo native: NVDA e gli altri lettori di schermo annunciano automaticamente titolo, tipo di controllo e stato attivato o disattivato, senza mostrare parole aggiuntive nel titolo e senza annunci vocali forzati.

Versione 0.8.3 – 2026-07-23

Modalità scura
• Aggiunta la modalità scura, attivabile dal menu Visualizza e salvata nelle preferenze.
• Il tema scuro viene applicato all’editor, ai menu, alle finestre secondarie e ai principali controlli di Sonarpad, adattando i colori del testo per mantenere leggibilità e accessibilità.

Lingua tedesca
• Aggiunta la lingua tedesca completa, selezionabile dalle Opzioni.
• Notizie e RSS, correttore ortografico, calendario e tutte le citazioni, donazioni, guida e changelog sono interamente disponibili in tedesco.

Portoghese brasiliano e Google News
• Aggiunto il portoghese brasiliano come lingua completa dell’interfaccia, separata dal portoghese del Portogallo e selezionabile dalle Opzioni.
• Interfaccia, calendario e citazioni, correttore ortografico, donazioni, guida e changelog sono interamente disponibili in portoghese brasiliano.
• Google News supporta ora la localizzazione del Brasile, le categorie brasiliane e fonti RSS brasiliane predefinite separate.
• Quando il feed le fornisce, le diverse fonti Google News relative alla stessa notizia vengono mostrate come elementi figli accessibili nell’albero.

TV in diretta
• Come già avveniva per RaiPlay, durante la riproduzione dei canali Rai della TV in diretta Sonarpad prova ora a selezionare automaticamente la traccia audiodescritta, quando disponibile.
• Velocizzata l’apertura della TV: i programmi in onda vengono caricati soltanto per la categoria, la regione, la ricerca o la pagina dei preferiti visualizzata e vengono riutilizzati temporaneamente.
• Corretto un problema per cui la pressione di Invio o Freccia destra utilizzata per aprire una categoria poteva propagarsi e avviare immediatamente il primo canale.

LibriVox
• Ottimizzata la ricerca di LibriVox per evitare richieste eccessive al servizio e blocchi dell’interfaccia. Sono state eliminate le scansioni estese del catalogo, ridotti i tentativi e introdotti tempi massimi più brevi.

Sintesi vocale
• Le sequenze di tre o più punti vengono ora normalizzate prima della lettura, evitando che alcune voci pronuncino “punto punto” o generino segmenti composti soltanto da punteggiatura.

Articoli correlati di Google News
• Per ogni notizia, quando disponibili, vengono ora mostrati gli articoli correlati, ossia altri articoli che trattano la stessa notizia. Per leggerli è sufficiente espandere l’articolo principale quando Sonarpad segnala che sono disponibili articoli correlati. Per chi non voglia espandere questa sezione, è sufficiente premere Invio sull’articolo principale e leggere la notizia come si è sempre fatto.
• Gli articoli correlati utilizzano ora lo stesso sistema letto/non letto degli articoli principali, compresi l’annuncio accessibile, la data e l’ora, il salvataggio dello stato e la sua conservazione dopo l’aggiornamento delle fonti o il riavvio di Sonarpad.

Annunci nelle parti degli audiolibri
• Aggiunta nelle Opzioni audio la casella combinata “Annuncio all’inizio di ogni parte”. Negli audiolibri suddivisi in più file è possibile non inserire alcun annuncio oppure far leggere all’inizio di ogni parte il titolo del libro, il titolo con il numero della parte, il nome del file o il nome del file con il numero della parte.

Versione 0.8.2 – 2026-07-17

Biblioteche digitali e audiolibri
• Aggiunto Project Gutenberg, con ricerca per titolo o autore e selezione della lingua.
• I libri EPUB di Project Gutenberg vengono scaricati nella cartella Documenti\Sonarpad\Documents; al termine Sonarpad chiede se aprire subito il libro nell’editor.
• Aggiunto Internet Archive per cercare e ascoltare raccolte audio, comprese radio d’epoca, discorsi e musica dal vivo.
• Aggiunto LibriVox per cercare audiolibri per titolo o autore e riprodurne direttamente i capitoli con lo stesso lettore utilizzato per i podcast.
• Le tre nuove funzioni sono disponibili nel menu Strumenti e, quando è attivo il raggruppamento dei menu, nella sezione Lettura.

Trascrizioni audio lunghe
• Corretta la trascrizione dei file audio lunghi: l’audio viene ora diviso automaticamente in parti da 15 minuti, trascritto una parte alla volta e poi riunito, evitando gli errori che potevano verificarsi con file di lunga durata.

YouTube
• Le azioni più utili che prima erano disponibili solo dopo aver aperto un video di YouTube e aver utilizzato il menu Riproduci sono ora disponibili anche direttamente nel menu contestuale dello stesso video, come “Trascrivi audio corrente”, “Crea audiodescrizione con IA” e “Salva media”, per una maggiore facilità di utilizzo.
• Aggiunta la voce “Copia link”, attivabile anche con Ctrl+C, per copiare negli appunti l’URL del video, della playlist o del canale YouTube selezionato.

Versione 0.8.1 – 2026-07-16

Sintesi vocale Google
• Corretto l’avvio di Google TTS nei sistemi Windows in cui le connessioni accettate dal server interno del browser ereditavano la modalità socket non bloccante, provocando l’errore 10035 e impedendo alle voci scaricate di parlare.
• Sonarpad attende ora che il motore WASM di Chrome o Edge sia realmente caricato prima dell’anteprima della voce o della lettura con F5, evitando l’errore “Chrome WASM TTS engine was not loaded”.
• Nel browser invisibile vengono disattivate la traduzione della pagina e l’accessibilità del renderer, evitando annunci come “Traduci pagina” e interferenze con i comandi di lettura.
• Nel pannello “Voci nell’editor” compare ora il pulsante “Gestisci voci Google...” quando è selezionato il motore Google; alla chiusura della gestione, l’elenco delle voci installate viene aggiornato immediatamente.
• Gli avvisi sulle dipendenze mostrati durante la rimozione dei pacchetti vocali Google sono ora tradotti in tutte le lingue dell’interfaccia.

Esperienza di aggiornamento
• Dopo un aggiornamento automatico, la finestra di completamento con il registro modifiche si apre dopo il ripristino iniziale del focus e resta in primo piano, invece di comparire soltanto dopo aver premuto Tab.

Documenti PDF
• Corretti i PDF in cui il testo incorporato conteneva caratteri NUL e veniva troncato alla prima occorrenza durante il caricamento nell’editor.
• Se pdf-extract restituisce NUL incorporati, Sonarpad riprova con PDFium; eventuali NUL residui vengono rimossi prima di inviare il testo ai controlli Windows, preservando il resto del documento.

Accessibilità dei menu
• Rimosso il calcolo delle mnemoniche durante l’esecuzione: le lettere di accesso sono ora scritte esplicitamente in ciascuna delle 15 traduzioni dell’interfaccia e restano quindi identiche a ogni avvio.
• Revisionate tutte le voci stabili dei menu principali e dei sottomenu, compresi Riproduzione, i caratteri, Salva immagine e Mostra indice EPUB; le mnemoniche mancanti o duplicate tra voci sorelle sono state corrette direttamente nelle traduzioni.
• I test automatici ora si limitano a controllare le traduzioni e falliscono se una mnemonica manca, non è valida o è duplicata; non modificano più le etichette durante l’esecuzione.
• Nei menu eccezionalmente estesi, quando il testo tradotto non contiene abbastanza caratteri distinti, viene mostrata una lettera di accesso numerica esplicita nella forma standard di Windows “(&1)”.

Versione 0.8.0 – 2026-07-15

Dizionario online
• Aggiunta la lingua tedesca al dizionario online Wiktionary.
• Le definizioni e i sinonimi tedeschi vengono ora riconosciuti correttamente secondo la struttura specifica del Wiktionary tedesco.

Enciclopedia Treccani
• Aggiunta una nuova funzione, disponibile nell’interfaccia italiana, per cercare e leggere le voci dell’Enciclopedia Treccani.
• È possibile scegliere un risultato, leggere l’intero articolo oppure una singola sezione.
• Dopo l’apertura del testo nell’editor, premendo Esc si torna alla casella di scelta delle sezioni, come nella funzione Wikipedia.
• Corretto il parser degli articoli: “Leggi tutto l’articolo” recupera ora il corpo completo della voce anche quando Treccani colloca indice e contenuto in contenitori HTML separati.
• Aggiunta la scorciatoia Ctrl+Shift+E per aprire rapidamente l’Enciclopedia Treccani.

Affidabilità degli audiolibri SAPI5
• La creazione degli audiolibri SAPI5 continua a utilizzare fino a 12 worker paralleli quando la voce selezionata produce risultati affidabili.
• Ogni parte generata viene ora controllata tramite dimensione del file, durata stimata e confronto prudenziale con il testo assegnato.
• Le parti mancanti o sospette vengono rigenerate automaticamente riducendo progressivamente la concorrenza: 12, 8, 6, 4, 2 e infine 1 worker. Vengono ripetute soltanto le parti problematiche.
• Il limite affidabile viene ricordato separatamente per ogni voce SAPI5, senza rallentare le voci che funzionano correttamente con 12 worker.
• Un controllo finale impedisce a Sonarpad di accettare silenziosamente un MP3 molto più corto delle parti generate.
• I dettagli della generazione e degli eventuali tentativi vengono registrati in `sapi5_audiobook_diagnostic.log`.
• Ogni unità di sintesi SAPI5 viene ora eseguita in un processo Sonarpad separato e invisibile. Se una voce di terze parti va in crash, si chiude soltanto quel worker e l’applicazione principale resta aperta.
• Nella stessa creazione dell’audiolibro, le parti non completate vengono riprovate immediatamente con il livello di concorrenza successivo più basso; le parti già validate vengono conservate.
• Il recupero al riavvio resta come protezione aggiuntiva soltanto se viene interrotta l’applicazione principale o il computer.

Worker degli audiolibri SAPI4
• Il numero di processi SAPI4 scelto dall’utente viene ora rispettato fino a un massimo tecnico di 64; è stato eliminato il precedente limite nascosto a 16.
• Il numero effettivo viene ridotto soltanto quando l’audiolibro contiene meno unità di lavoro rispetto ai processi richiesti.
• Se uno o più processi del bridge SAPI4 falliscono, le parti completate vengono conservate e soltanto le unità non riuscite vengono riprovate automaticamente con una concorrenza progressivamente inferiore.
• Sonarpad controlla ora il codice di uscita del bridge SAPI4 e rifiuta le parti audio vuote o non valide invece di considerarle completate.

Configurazione del proxy
• Aggiunto nelle impostazioni di rete un campo separato per la porta del proxy.
• La porta può ora essere indicata indipendentemente dall’indirizzo, viene convalidata nell’intervallo da 1 a 65535 e sostituisce correttamente un’eventuale porta già presente nell’URL.

Ricerca radio per lingua e nazione
• I filtri Lingua e Nazione vengono ora aggiornati con tutte le voci disponibili nel catalogo Radio Browser, senza essere più limitati a un elenco fisso.
• I nomi delle lingue vengono ora riconosciuti anche quando Radio Browser li fornisce in un altro alfabeto, nella forma nativa, come abbreviazioni o come combinazioni di più lingue, e vengono mostrati tradotti nella lingua attuale dell’interfaccia. I valori che non rappresentano vere lingue, come numeri, generi musicali, nazioni o diciture generiche, vengono esclusi.
• L’aggiornamento avviene in sottofondo, mantenendo un elenco di riserva utilizzabile anche quando Radio Browser non è raggiungibile.
• Le voci di Radio Browser che, dopo la traduzione, risultano identiche vengono ora unite in un solo elemento della casella combinata, evitando passaggi muti con gli screen reader.

Miglioramento principale: sincronizzazione tra lettura vocale e cursore
• La sincronizzazione tra la lettura vocale e lo spostamento del cursore è stata migliorata in maniera significativa per tutti i motori di sintesi vocale supportati.
• Quando è attivata l’impostazione “Sposta il cursore durante la lettura”, Sonarpad utilizza ora un sistema di avanzamento comune per Microsoft Edge Neural, Google TTS, SAPI4, SAPI5 e OneCore.
• Il cursore segue con maggiore precisione il testo effettivamente pronunciato, con una suddivisione più coerente per frasi e parti di frase.
• Ridotti sensibilmente anticipi, ritardi e salti irregolari del cursore, oltre alle differenze di comportamento tra un motore vocale e l’altro.
• Migliorato il mantenimento della posizione corretta dopo pause, riprese della lettura, ricerche nel documento e cambiamenti del motore vocale.

Registrazione podcast su due tracce separate
• Aggiunta l’opzione “Salva microfono e audio di sistema o delle app in due file separati”.
• Quando microfono e un’altra sorgente vengono registrati insieme, Sonarpad può ora creare un file contenente soltanto il microfono e un secondo file contenente l’audio del sistema, di una singola applicazione oppure delle applicazioni selezionate.
• La separazione delle sorgenti è disponibile sia in MP3 sia in WAV.
• Se l’opzione non viene attivata, Sonarpad continua a produrre normalmente un unico file con le sorgenti miscelate.
• Le tracce separate facilitano la regolazione dei volumi, la rimozione dei rumori, la correzione di una singola sorgente e il montaggio successivo di podcast, interviste e tutorial.

TV in diretta
• Aggiunta una nuova sezione dedicata alla televisione in diretta.
• È possibile esplorare i canali nazionali e regionali organizzati per categoria, cercare rapidamente un canale e gestire i preferiti.
• Aggiunte le informazioni sul programma attualmente in onda e la consultazione della guida TV.
• I canali possono essere riprodotti con o senza finestra video.
• È possibile registrare una trasmissione oppure registrarla continuando contemporaneamente ad ascoltare il canale.
• Aggiunto l’accesso diretto all’elenco delle registrazioni TV.
• Migliorata la corrispondenza tra l’orario corrente, la guida TV e il programma mostrato come “Ora in onda”, evitando la visualizzazione di programmi precedenti già terminati.
• Resa più affidabile la riproduzione dei canali anche quando il video è disattivato.

Registrazioni programmate
• Aggiunta la possibilità di programmare in anticipo le registrazioni di TV e radio.
• Per ogni registrazione è possibile scegliere il canale o la stazione, il giorno, l’ora e i minuti di inizio e la durata.
• È disponibile una durata personalizzata compresa tra 1 e 1.440 minuti.
• La registrazione può essere eseguita una sola volta, ogni giorno oppure ogni settimana.
• La sezione delle registrazioni mostra con maggiore chiarezza le registrazioni in corso, quelle programmate, la data e l’ora previste, la durata e il tempo rimanente prima dell’avvio.
• Le registrazioni programmate possono essere gestite mediante l’Utilità di pianificazione di Windows, consentendo l’avvio automatico anche quando Sonarpad non è già aperto.

Calendario
• Aggiunto un calendario completo e accessibile da tastiera.
• È possibile consultare i giorni precedenti e successivi, tornare rapidamente alla data di oggi e conoscere festività e ricorrenze.
• Aggiunti il santo del giorno e la citazione del giorno, che possono essere letti, ascoltati o copiati.
• È possibile aggiungere, modificare e cancellare promemoria.
• I promemoria possono essere segnalati all’ora esatta oppure con anticipo, posticipati temporaneamente e segnati come completati.
• I promemoria con data e ora possono utilizzare la programmazione di Windows, così da essere segnalati anche quando Sonarpad non è aperto.

Meteo
• Aggiunta una nuova sezione dedicata alle previsioni meteorologiche.
• È possibile cercare una città e richiamare rapidamente le località consultate di recente.
• Sono disponibili situazione attuale, temperatura, valori minimi e massimi, umidità, probabilità di precipitazioni e previsioni dei giorni successivi.
• È possibile scegliere tra gradi Celsius, Fahrenheit oppure selezione automatica.

Film al cinema
• Aggiunta una sezione per consultare i film attualmente nelle sale e le prossime uscite.
• Sono disponibili ricerca per titolo, trama, data di uscita e riproduzione del trailer.

Sintesi vocale Google
• Integrato un nuovo motore di sintesi vocale Google, utilizzabile per la lettura dei documenti e per la creazione degli audiolibri.
• Aggiunto un gestore delle voci per visualizzare le voci disponibili, filtrarle per lingua, scaricarle ed eliminare quelle non più necessarie.
• È possibile controllare velocità, volume e tono.
• Per le voci Google Natural, il tono viene applicato direttamente dal motore, ottenendo un risultato più naturale e stabile.
• Migliorate reattività e affidabilità di Google TTS, con tempi massimi di sintesi adattati alla velocità della voce.
• Ridotti i tempi di attesa inutili quando il motore non risponde e migliorata la gestione di errori e interruzioni.
• Resa più stabile la scrittura del registro diagnostico durante operazioni simultanee.

Indice dei documenti EPUB
• Sonarpad riconosce ora l’indice incorporato nei libri EPUB.
• La presenza dell’indice viene annunciata ed è possibile aprirlo dal menu Visualizza.
• Capitoli e sottocapitoli vengono mostrati in modo gerarchico.
• Premendo Invio si raggiunge immediatamente il punto scelto del libro.

Notizie e fonti RSS
• Ampliata la sezione Notizie con nuove possibilità di ricerca e organizzazione.
• Aggiunta la scelta della lingua delle notizie.
• È possibile cercare all’interno delle fonti RSS e consultare le notizie della propria città.
• Aggiunta la possibilità di esplorare le fonti condivise dalla comunità Sonarpad, aggiungerle alla propria raccolta e inviare nuove fonti RSS alla comunità.

Registrazione podcast
• È possibile registrare soltanto il microfono, tutto l’audio del sistema, una singola applicazione, più applicazioni selezionate oppure microfono e applicazioni contemporaneamente.
• È possibile scegliere il dispositivo del microfono e la sorgente audio, regolare separatamente i volumi e controllare in tempo reale il livello delle sorgenti.
• Aggiunti pausa e ripresa della registrazione, scelta tra MP3 e WAV, selezione del bitrate MP3 e della cartella di salvataggio.
• È possibile mantenere il computer attivo durante la registrazione.
• I file separati ricevono automaticamente nomi distinti, rendendo immediatamente riconoscibile la traccia del microfono rispetto a quella del sistema o delle applicazioni.

Radio
• La sezione Radio è stata profondamente riorganizzata.
• È possibile cercare le stazioni per nome o testo libero, lingua, nazione, città e genere musicale o categoria.
• Migliorata la gestione dei preferiti e aggiunta la possibilità di azzerare rapidamente tutti i filtri.
• È possibile aggiungere una stazione alla comunità Sonarpad.
• Aggiunte la registrazione della diretta, la modalità “Registra e riproduci”, l’elenco delle registrazioni e la loro cancellazione e gestione.
• Le registrazioni TV e radio vengono conservate in cartelle separate all’interno della cartella generale delle registrazioni.

Riproduzione multimediale
• Migliorata in modo sostanziale la stabilità del riproduttore multimediale.
• Corretto un problema che poteva bloccare mpv e resa più affidabile la comunicazione con il riproduttore.
• Migliorata l’apertura dei diversi tipi di file multimediali.
• Sonarpad ricorda ora il livello del volume utilizzato durante la riproduzione.
• Migliorata la gestione degli stream e delle registrazioni e corretto il ritorno alle finestre di TV e radio dopo la riproduzione.
• Corretta l’apertura dei file inviata a Sonarpad direttamente da Windows, ad esempio tramite doppio clic o “Apri con”.
• Migliorata la gestione degli stream MediaKit sia in modalità audio sia con il video attivo.

Documenti PDF
• Aggiunto il riconoscimento dei campi presenti nei moduli PDF.
• Sonarpad può individuare i campi compilabili, presentarli in forma testuale accessibile, permetterne la modifica e salvare nel PDF i dati inseriti.
• Corretto il calcolo della posizione del cursore durante la lettura vocale, soprattutto nei documenti con caratteri multibyte o strutture complesse.
• Il nuovo sistema comune di sincronizzazione migliora ulteriormente lo spostamento del cursore con tutti i motori vocali.

Accessibilità e comandi da tastiera
• Migliorato il funzionamento dei normali comandi di modifica nelle diverse finestre del programma.
• Copia, taglia, incolla, seleziona tutto, annulla e ripristina vengono ora inviati correttamente al campo che possiede il focus, anche nelle finestre secondarie e di dialogo.
• Corretto un problema che poteva impedire al display Braille di aggiornarsi correttamente.
• Migliorata la gestione del focus nelle finestre secondarie.
• Corretta la selezione della lingua nella finestra di Wikipedia.
• Aggiunta la possibilità di raggruppare le funzioni del menu Strumenti per categoria.
• Aggiunte azioni configurabili per aprire rapidamente Calendario, Meteo e Film al cinema.
• Migliorata la visualizzazione del changelog al termine di un aggiornamento.

Audiolibri
• Migliorata la gestione della creazione degli audiolibri quando sono aperte finestre di dialogo o altre finestre modali.
• Il controllo dell’avanzamento è più robusto e ignora gli aggiornamenti audio non più validi, riducendo blocchi, notifiche errate e finestre che non rispondono.
• Google TTS può essere utilizzato anche per creare audiolibri, con controllo di velocità, volume e tono.

Intelligenza artificiale
• Aggiornato il modello Gemini utilizzato come impostazione predefinita a `gemini-3.5-flash`.

Correzioni generali
• Risolti diversi blocchi durante la riproduzione con mpv.
• Corretta l’apertura di alcuni file audio e video.
• Migliorata la gestione dei comandi inviati al riproduttore.
• Corretto il ripristino del cursore durante la lettura vocale.
• Corretto il funzionamento delle scorciatoie nei campi di testo delle finestre ausiliarie.
• Migliorata la stabilità della creazione degli audiolibri.
• Corretta l’apertura di file richiesta dall’esterno tramite Windows.
• Migliorata la gestione delle registrazioni TV e radio.
• Migliorata la gestione complessiva di media, RSS, TV ed EPUB.

Versione 0.7.1 – 2026-05-13

Novità e miglioramenti
• Creato il sito ufficiale sonarpad.com, un nuovo punto di riferimento per seguire le ultime novità, scaricare l’ultima versione del programma, leggere i commenti dei visitatori e, in futuro, ascoltare anche tutti i podcast di Sonarpad. Nel menu Aiuto è stata aggiunta anche la voce “Visita sonarpad.com”, per aprire rapidamente il sito ufficiale.
• Corretto il problema per cui i file con accenti o caratteri speciali davano errore quando veniva avviata la trascrizione vocale.
• Da ora, nel menu Visualizza, le voci come A capo automatico e Mostra video durante la riproduzione appariranno sempre con lo stato corretto, attivate o disattivate.
• Migliorata la ricerca in YouTube, permettendo di tornare con Esc alla pagina o schermata precedente.
• Aggiunto un controllo preliminare per verificare se un video è riproducibile. Migliorata anche la riproduzione: Sonarpad ora può riprodurre anche video o playlist contrassegnati come mix, che prima non venivano riprodotti.
• Migliorata la gestione dei segnalibri automatici. Prima, se l’opzione Segnalibri automatici era attiva e poi veniva disattivata, quei segnalibri rimanevano; ora il programma li ignora correttamente finché l’opzione non viene riattivata. Inoltre, quando si arriva alla fine di un file multimediale, il segnalibro viene cancellato automaticamente.
• Corretto il problema per cui su RaiPlaySound non veniva salvato il segnalibro automatico quando l'opzione era attiva.
• Su RaiPlaySound è stata aggiunta nel menu contestuale la possibilità di inserire il podcast tra i preferiti, direttamente nella libreria dei podcast. Così si resterà aggiornati anche sulle nuove puntate. Occorre andare sulla puntata interessata e premere il tasto Applicazioni: si troverà la voce Aggiungi ai podcast.
• Nel download di un MP3 da RaiPlay viene preferita la traccia audiodescritta, quando disponibile.
• Da ora, accedendo alla biblioteca BdCiechi e scaricando un libro, si verrà sempre aggiornati su quanti download rimangono nel mese corrente.
• Migliorata la gestione dei tag con i dialoghi attivi. Ora Sonarpad gestisce correttamente entrambe le funzioni, permettendo di inserire i tag anche se l’opzione dialoghi è attiva.
• Migliorate le impostazioni voce, separando chiaramente ogni motore, così la regolazione è più precisa. I profili voce conservano correttamente le impostazioni per ogni singolo motore: Edge, Sapi5 e Sapi4.
• Aggiunto un tag per inserire pause, direttamente dalle opzioni o dal pannello voci premendo Tab dall’editor. Le scelte sono: 250 ms, 500 ms, 1 secondo, 2 secondi o durata personalizzata.
• Corretto il comportamento quando si riproduce un video da YouTube e si avvia la trascrizione. Ora, tornando con Alt+Tab, il focus sarà correttamente sul pulsante Annulla della trascrizione in corso.
• Da ora le trascrizioni vengono salvate automaticamente al termine del processo.
• Migliorata l’importazione da Wikipedia. Si può scegliere se leggere soltanto una sezione e poi, dall’articolo, premendo Esc si tornerà alla ricerca, oppure importare tutto l’articolo. Si può scegliere anche la lingua di Wikipedia da consultare.
• Aggiunta una sezione con le radio da tutto il mondo, dove si potrà cercare una radio in base a paese, lingua e genere. Si potranno anche aggiungere radio locali al database di Sonarpad, così anche gli altri utenti potranno ascoltarle. È possibile anche aggiungere una radio ai preferiti.
• Aggiunta una sezione sui percorsi stradali per calcolare percorsi scegliendo il mezzo: a piedi, in bici, in auto e in sedia a rotelle. Si può scegliere se calcolare il percorso più breve o più veloce e se mostrare i comuni attraversati. Una volta importato il percorso, si potrà anche salvare la mappa visiva dal menu File, Salva immagine.
• Aggiunta la voce Stampa nel menu File. Sonarpad stamperà i file TXT usando il programma stesso e userà il programma associato per gli altri file, come DOCX, PDF e simili, così da preservare il più possibile il layout originale.
• Integrato in Sonarpad un servizio di traduzione per ogni documento, accessibile dal menu contestuale dell’editor. L’utente potrà usare senza inserire alcuna API key i servizi gratuiti DeepL e Google Translate; inserendo invece una API key Gemini, potrà tradurre usando Gemini.
• Nel menu di traduzione l’utente potrà scegliere la lingua di destinazione. Il menu si riordina automaticamente: se un utente sceglie prima inglese, poi francese e poi italiano, queste tre opzioni saranno mostrate in cima al menu delle lingue.
• Se l’utente inserisce la propria API key Gemini, potrà inoltre accedere alla funzione Riassumi testo, sempre presente nel menu contestuale, per riassumere qualunque articolo.
• Aggiunto nel menu Riproduci, visibile quando si riproduce un file multimediale, un menu per dividere il media corrente. Funziona con MP3, MP4 e altri formati, dividendo per numero di parti oppure in base alla durata di ogni parte.

Versione 0.7.0 – 2026-04-25

Novità
• Aggiunto il supporto al player mpv per la riproduzione streaming. I video da YouTube e da siti supportati vengono ora riprodotti immediatamente; se l'utente sceglie di conservarli, vengono scaricati come in precedenza. Se si avvia la trascrizione di un contenuto streaming, questo viene prima scaricato e poi trascritto. Il player mpv è ora utilizzato anche per aprire video locali e per la gestione dei sottotitoli, garantendo una maggiore compatibilità con numerosi formati che prima non erano gestiti al meglio.
• Migliorata la registrazione podcast dell'audio di sistema: ora è possibile scegliere se registrare tutto l'audio di sistema, una singola applicazione oppure più applicazioni contemporaneamente. Questa scelta è integrata con la registrazione normale, quindi è comunque possibile attivare o disattivare il microfono separatamente.
• Aggiunta la lingua Hindi. Interfaccia tradotta, aggiunti RSS, changelog e guida di Sonarpad.
• Aggiunta un'opzione nella scheda Editor per spostare il cursore sempre all'inizio della riga usando le frecce su e giù.
• Aggiunta nella voce di menu "Converti audio" l'opzione per convertire un audio in M4B.

Correzioni
• Corretta la lettura degli articoli dal Corriere della Sera aggiornando la fonte RSS. Da ora gli articoli saranno di nuovo sempre aggiornati.
• Corretto il tasto `F10`, che ora torna a passare alla voce preferita successiva durante la lettura del testo.
• Quando è in corso una registrazione podcast, chiudendo un altro documento non viene più chiusa anche la registrazione attiva.
• Nei commenti YouTube aperti da "Riproduci audio da streaming...", Sonarpad ora carica inizialmente solo i primi 50 commenti principali, includendo sempre tutte le risposte di quei commenti, e aggiunge in fondo una voce per caricare tutti i commenti su richiesta.
• I segnalibri ora vengono mostrati e gestiti in ordine di posizione sia nei documenti di testo sia nei file multimediali, invece di seguire l'ordine di creazione. Se un segnalibro esiste già nella stessa posizione, non viene più aggiunto di nuovo.
• Migliorata la gestione in "Tutte le audiodescrizioni": ora i film sono separati dalle fiction tramite un pulsante `Film` raggiungibile con Tab, che raccoglie tutti i film audiodescritti. La ricerca trova ora risultati sia nelle fiction sia nei film.
• In Pagine Bianche è stato aggiunto un pulsante, raggiungibile con Tab, per esplorare tutti i risultati della pagina corrente.
• In RaiPlay, durante il salvataggio, è disponibile una nuova opzione `MP4 con audiodescrizione`, che forza il salvataggio in formato MP4 usando la traccia audiodescritta.
• Aggiunta un'opzione nel menu Segnalibri che, se attivata, permette una gestione automatica dei segnalibri. Quando si riproduce un file locale o in streaming e lo si chiude, Sonarpad imposta automaticamente un segnalibro in base alla posizione raggiunta e, alla riapertura, riprende da quel punto. La stessa cosa avviene per i file di testo: se si apre un testo e si sposta il cursore, alla chiusura Sonarpad ricorderà quella posizione; se invece si avvia la lettura, verrà registrata l'ultima frase letta e la lettura ripartirà esattamente da lì.
• Aggiunta nel menu Visualizza la voce per mostrare il rendering video per i file locali o in streaming. Il contenuto video viene mostrato in una finestra ingrandita, dove tutti i comandi sono nascosti, tranne quando si preme il tasto Alt o si porta il mouse verso la parte superiore della finestra. In questo modo gli utenti ipovedenti dovrebbero avere un contenuto più grande e più fruibile.

Versione 0.6.9 – 2026-04-08

Correzioni
• Migliorata l'esperienza di Trova nei file: quando si apre Sfoglia cartella il focus viene subito posizionato sulla visualizzazione elenco; aprendo un risultato con Invio tutti i comandi da tastiera continuano a funzionare; premendo Esc si torna al risultato precedentemente selezionato; e tornando con Alt+Tab il focus viene portato al campo di ricerca oppure ai risultati, se questi erano aperti.
• F5 avviava sempre la lettura dall'inizio. Ora è stato corretto e la lettura parte dal punto in cui si trova il cursore, preservando anche `Shift+F5` e `Ctrl+F5` per andare alla frase precedente o successiva.
• Dopo essere andati a Vai alla riga, premendo Esc si usciva da Sonarpad. Ora il focus torna correttamente nell'editor.
• L'opzione `A capo automatico` ora viene applicata subito anche ai documenti già aperti, senza dover riaprire il file.

Versione 0.6.8 – 2026-04-07

Novità
• Nuova voce nel menu Riproduci per trascrivere qualsiasi file audio o video con Whisper. Nelle Opzioni è disponibile una nuova sezione “AI e trascrizione”, con scelta del modello, supporto opzionale CUDA (schede video NVIDIA), opzione per mantenere la lingua originale e attivazione/disattivazione dei timestamp.
• Aggiunta nel menu Riproduci la nuova azione `Trascrivi cartella corrente`, che trascrive tutti i file audio supportati presenti nella cartella del media aperto e li unisce in un unico documento, con finestra di avanzamento dedicata, indicazione del file corrente e possibilità di annullare. Si può richiamare anche con la scorciatoia `Alt+Shift+C`.
• Aggiunta la possibilità di usare la dettatura vocale offline, con le stesse modalità della trascrizione audio. Per impostazione predefinita si preme `Ctrl+Shift+Spazio` per avviare la dettatura e si preme la stessa scorciatoia per terminarla; il tasto rapido è personalizzabile nelle Opzioni. Dalla seconda attivazione la dettatura risulta più veloce, perché il motore resta già pronto in memoria; su PC con meno di 4 GB di RAM questo precaricamento e riutilizzo vengono disattivati automaticamente.
• Aggiunta nelle Opzioni dell'editor una nuova impostazione, disattivata per default, che fa chiudere la finestra dell'editor con `Esc`.
• Aggiunta una nuova sezione per visualizzare e gestire tutti i video di RaiPlay, con gestione integrale di tutti i contenuti, comprese le dirette, i contenuti in evidenza e la ricerca in tutto il catalogo.
• Aggiunta la gestione di RaiPlay Sound, con esplorazione del catalogo, ricerca globale dei contenuti e riproduzione di tutti i podcast disponibili, compresi i GR e il teatro.
• Inserita una nuova sezione per ricercare tutti i nominativi in Pagine Bianche e Pagine Gialle, con possibilità di inserire nome, città e indirizzo (facoltativo).
• La ricerca podcast ora usa di default `iTunes + Spreaker`, con filtro dei risultati duplicati quando lo stesso podcast è presente su entrambe le piattaforme.
• Migliorata la ricerca e l'esplorazione dei podcast Apple: la ricerca podcast, la navigazione per categoria e i top podcast per categoria ora usano il paese selezionato per la directory podcast. In Opzioni > RSS / Podcast si può lasciare `Automatico` per usare il paese del sistema oppure scegliere manualmente un altro paese.
• Aumentato il limite dei risultati per le categorie podcast Apple. Alla prima apertura vengono caricati i primi 50 risultati come sempre; se si sceglie `Carica altri risultati`, Sonarpad carica fino a 200 risultati totali (limite imposto da Apple) e permette di navigare nelle pagine successive mantenendo un'esperienza più fluida.
• Sonarpad è disponibile anche su Mac, anche se con un set di funzioni parziale. Link al progetto: https://github.com/Ambro86/Sonarpad-Mac

Miglioramenti
• Aggiunte più di 50 nazioni selezionabili per la directory dei podcast, così è possibile scegliere tra molti più cataloghi nazionali.
• "Riproduci audio da streaming..." ora permette anche di cercare su YouTube scrivendo una qualunque stringa di testo oppure di incollare il link di un canale o di una playlist YouTube per visualizzarne i risultati.
• Migliorata la visualizzazione dei risultati in "Riproduci audio da streaming...": le voci YouTube ora includono titolo, durata, canale e visualizzazioni in un formato più chiaro.
• "Riproduci audio da streaming..." ora supporta anche i commenti di YouTube: si possono aprire dal menu contestuale, leggere le risposte ed espandere i thread dei commenti con la Freccia destra.
• Aggiunta in "Riproduci audio da streaming..." la possibilità di salvare canali e playlist YouTube nei preferiti: si possono aggiungere dai risultati tramite menu contestuale, aprire direttamente dalla lista Preferiti raggiungibile con Tab subito dopo il campo URL/query YouTube e rimuovere sempre dalla stessa lista tramite menu contestuale. Nei risultati della ricerca YouTube il menu contestuale è disponibile solo per canali e playlist.
• In "Riproduci audio da streaming..." ora, quando un sito richiede l'accesso, Sonarpad può chiedere le credenziali. L'utente può inserirle, salvarle per il sito e gestire in seguito le credenziali salvate da Opzioni > Audio.
• Migliorato il focus durante "Riproduci audio da streaming...", così la finestra di avanzamento resta più stabile durante il download e la conversione.
• Aggiunte nel menu Voce due nuove azioni per la lettura: `Frase precedente` e `Frase successiva`, con scorciatoie personalizzabili per saltare durante la lettura del testo.
• La scorciatoia predefinita di `Esegui file con interprete` è ora `Ctrl+Shift+F5`, così `Shift+F5` può essere usata di default per `Frase precedente`.
• Ora il programma riesce a gestire Sostituisci tutto in modo massivo su file grandi con un gran numero di sostituzioni.
• Aggiunto il pieno supporto della biblioteca BdCiechi, accessibile da Strumenti o con la scorciatoia Alt+Shift+B. E' possibile cercare, scaricare libri, leggere le ultime novità, scaricare un testo di assaggio. Quando verrà salvato un file è possibile aprirlo direttamente in Sonarpad e leggerlo con le voci di alta qualità presenti nel programma.
• Diversi miglioramenti alla biblioteca BdCiechi grazie alla collaborazione con Giuliano Artico: login e password possono essere memorizzati in Sonarpad per 30 giorni, dopo i quali è necessario effettuare nuovamente l'accesso; se invece l'utente sceglie di non memorizzare i dati, finché Sonarpad resta aperto questi vengono riutilizzati senza doverli digitare di nuovo. La ricerca delle opere è stata normalizzata correggendo gli accenti non immessi, quindi ad esempio cercando `giosue` viene trovata anche `Giosuè`. Corretto inoltre un problema per cui, alla riapertura di BdCiechi, la finestra non si riattivava correttamente. Il catalogo della BdCiechi viene ora scaricato solo al primo utilizzo e aggiornato successivamente solo quando necessario. Aggiunto anche un pulsante per disconnettersi dalla biblioteca e, nel nome del file salvato, l'autore viene ora inserito prima del titolo del libro.
• Migliorata la finestra RSS con un'anteprima articolo integrata, così il testo può essere consultato direttamente lì e raggiunto rapidamente con Tab prima di aprire l'articolo completo nell'editor.
• Aggiunta negli RSS una voce esplicita “Carica altre notizie” in fondo alle fonti quando sono disponibili altri elementi; premendo Invio viene caricato il blocco successivo e il focus si sposta sulla prima notizia nuova.
• Aggiunto il supporto per le audiodescrizioni Rai. Si possono visualizzare le audiodescrizioni più recenti e tutte le audiodescrizioni ordinate per categoria. Per accedere al servizio è necessario richiedere un codice all'autore. In maniera automatizzata il programma proporrà l'invio della mail. In caso di problemi è sufficiente inviarla ad ambro86@gmail.com con oggetto `Richiesta codice Sonarpad`, con nel corpo della mail il proprio nome e cognome. Tutto questo è necessario per evitare abusi del servizio. Aggiunto anche il menu contestuale per copiare negli appunti l'indirizzo streaming delle audiodescrizioni.
• Aggiunta in Opzioni > Voce la gestione dei profili voce: è possibile aggiungere, rinominare ed eliminare un profilo.
• Nel dizionario vocale, quando si aggiunge o modifica una sostituzione, è ora disponibile la casella “Distingui maiuscole e minuscole”, che permette di scegliere se applicare la sostituzione rispettando o ignorando il maiuscolo/minuscolo.
• Ampliate in Opzioni > Audio le scelte per l'intervallo di riavvolgimento durante la riproduzione, con nuovi valori da 1 secondo fino a 2 ore.
• Aggiunta la traduzione russa grazie a Dmitriy.
• Aggiunta in Opzioni > Audio una nuova scelta per il formato nome delle parti audiolibro: `Titolo + numero`, `Solo numero` oppure `Numero + titolo`.
• Aggiunta nel menu contestuale degli articoli RSS la voce per aggiungere l'articolo ai preferiti.
• Introdotta la fonte RSS "Preferiti": può essere eliminata e viene ricreata automaticamente alla prossima aggiunta di un articolo ai preferiti.
• Aggiunte le scorciatoie da tastiera RSS per spostare le fonti in alto/in basso: `Ctrl+Shift+Freccia su` e `Ctrl+Shift+Freccia giù`.
Correzioni
• Ora "Riproduci audio da streaming..." e la riproduzione delle audiodescrizioni rispettano il limite cache già impostato per i podcast.
• Corretta l'importazione da Wikipedia, che in alcune pagine non riportava correttamente le citazioni presenti nel testo.
• Migliorato il parser delle pagine web: in alcune pagine WordPress non venivano inclusi gli elementi delle liste e alcuni titoli di sezione.
• Ora, usando "Vai alla riga", il campo viene precompilato con la riga attuale.
• Corretta l'esportazione OPML di podcast e RSS, che ora genera file accettati da iTunes.
• Aggiunti messaggi di conferma localizzati per la corretta importazione ed esportazione OPML di feed RSS e podcast.
• Corretto un problema per cui, in "Riproduci audio da streaming...", scrivendo una stringa di ricerca e selezionando un canale YouTube dai risultati il programma poteva sembrare bloccato invece di aprire i video del canale.
• Corretta la trascrizione dei file media: ora, chiudendo con Alt+F4 il documento generato, Sonarpad chiede se si vuole salvare il file e propone il nome corretto basandosi sul nome del file trascritto, invece che sulla prima riga del testo.
• Corretto un bug per cui l’elenco dei file aperti veniva mostrato nel menu Aiuto invece che nel menu Finestra.
• Corretto un caso limite nello streaming in cui la riproduzione poteva partire ma la finestra “Download streaming” restava aperta quando il file scaricato era già nel formato di destinazione.
• Corretto il comportamento di conversione nello streaming MP3: quando lo stream è già MP3 e l’utente sceglie un bitrate MP3 esplicito (ad esempio 128 kbps), Sonarpad ora ricodifica al bitrate selezionato invece di saltare la conversione.
• Corretta la scorciatoia `Alt+Shift+L`: ora apre correttamente la lista capitoli durante la riproduzione.
• Corretta la scorciatoia `Alt+Shift+T`: ora avvia correttamente “Trascrivi audio corrente” invece di aprire il menu Strumenti.
• Corretto il comportamento del tasto `.` nel menu Riproduci: ora equivale a Stop e ferma solo il brano corrente, senza uscire dal player o dall’episodio.
• Corretta la voce di salvataggio nel menu Riproduci per i media aperti da File recenti: quando il file proviene da una cache locale di Sonarpad, l'opzione localizzata per salvare il file viene ora mostrata correttamente anche in questo caso.
• Se è già in corso la riproduzione di un audio, quando si avvia la trascrizione Sonarpad mette automaticamente quell’audio in pausa prima di iniziare.
• Corretto un problema per cui, importando un articolo da Wikipedia, l’importazione poteva riuscire ma il testo dell’articolo non veniva mostrato sullo schermo.
• Aggiunto il supporto ai capitoli podcast embedded nei file multimediali locali (es. metadati capitoli MP3): quando feed/URL non forniscono capitoli, Sonarpad li legge dal file scaricato in background, così la riproduzione parte subito e i capitoli vengono applicati appena disponibili.
• Corretta la lettura dei capitoli per gli episodi podcast scaricati e aperti come normali file multimediali locali: i capitoli embedded sono ora disponibili anche in questo caso, non solo avviando la riproduzione dalla finestra Podcast.
• Corretta la finalizzazione degli audiolibri MP3 con SAPI4 e SAPI5: il file finale viene ora finalizzato correttamente, evitando file incompleti o fragili dopo esportazioni lunghe.
• Aggiunta una barra di progresso esplicita per la fase di finalizzazione in tutte le modalità di creazione degli audiolibri: dopo la creazione, Sonarpad annuncia e mostra la finalizzazione con avanzamento visibile.
• Corretto un bug nelle voci dialoghi: i parametri di velocità/tono/volume della prima e della seconda voce dialoghi ora vengono applicati correttamente durante la sintesi.
• Migliorato il rilevamento codifica per file `.txt` giapponesi: aggiunto fallback sicuro Shift_JIS/CP932 nei casi di mojibake, preservando il comportamento esistente su UTF/diacritici/cinese.
• Refactor interno sulla sicurezza: conversione a implementazioni safe dove possibile e riduzione drastica delle righe di codice unsafe.

Versione 0.6.7 – 2026-03-02
Miglioramenti
• Aggiornata la traduzione polacca grazie a DJ Graco.
• Aggiunta la traduzione lituana.
• Aggiunta la traduzione cinese.
• D’ora in poi, build beta frequenti saranno pubblicate nella sezione Releases del progetto, così gli utenti potranno testare le nuove modifiche prima della prossima versione stabile.
• Aggiunta la scorciatoia `Ctrl+.` per inserire il carattere di ellissi (…).
• Migliorato il supporto ai capitoli podcast: la navigazione capitoli è ora più affidabile anche negli episodi diretti/streaming in cui i capitoli non sono incorporati nel file MP3, usando quando disponibili i metadati capitolo dal feed/URL come fallback. Aggiunte le scorciatoie `Ctrl+Alt+Pagina su` (capitolo precedente) e `Ctrl+Alt+Pagina giù` (capitolo successivo).
• Riorganizzate le cartelle di output in `Documenti\\Sonarpad`: i file ora vengono salvati nelle sottocartelle dedicate `audiobooks`, `documents`, `recordings` e `media`, con migrazione automatica dai percorsi legacy.
• Migliorato il supporto per file di testo molto grandi (anche 60 MB): apertura e navigazione riga per riga più fluide, in particolare con gli screen reader.
• Aggiornate le guide per tutte le lingue e aggiornate le risorse di localizzazione dell'app, incluse testo donazioni e traduzioni setup NSIS (nuove stringhe installer in cinese semplificato e lituano, più completamento della traduzione ucraina del setup).
• Aggiunto il supporto proxy di rete globale (HTTP/HTTPS e SOCKS5/SOCKS5H) per le funzioni online, con validazione al salvataggio Opzioni: i proxy non validi vengono segnalati e rimossi automaticamente.
• Aggiunta una nuova funzione in Strumenti: "Riproduci audio da streaming...", che permette di inserire un URL (YouTube o link media diretto), scegliere il formato di output e il profilo qualità/bitrate (inclusa qualità/bitrate originale per MP3 e MP4) e avviare la riproduzione nell’audio player di Sonarpad.
• Aggiunto il supporto al tasto multimediale Play/Pausa di sistema (cuffie/tastiera): ora controlla sia la riproduzione media sia la pausa/ripresa della lettura testo (con priorità al player media quando entrambi sono attivi).
• Aggiunta nel menu File > File recenti la nuova voce "Svuota file recenti" per cancellare rapidamente l’elenco dei documenti recenti.
• Ampliate le opzioni di bitrate nella conversione audio e nella registrazione podcast: aggiunti valori più bassi (64/96 kbps) ed esteso MP3 fino a 320 kbps, con validazione e gestione encoder allineate.
• Estese le opzioni di divisione audiolibro in base al tempo fino a 60 minuti.
• Migliorata la divisione audiolibro in parti: ora il numero di parti è inseribile manualmente, con validazione da 1 a 100.
• Aggiunta la nuova modalità Visualizza > Sola lettura per bloccare modifiche accidentali nel testo mantenendo piena lettura e navigazione dei documenti.
• Aggiunta una barra di progresso accessibile durante gli aggiornamenti del programma, così i lettori di schermo possono seguire in tempo reale l’avanzamento del download.
• Aggiunta una nuova barra di stato discreta nella finestra principale con conteggio caratteri, parole e posizione riga/colonna (esempio: "Caratteri (con spazi): 11. | Parole: 2. | Ln 1, Col 12"), senza interferire con il focus di NVDA.
• Aggiunta nel menu Visualizza la nuova voce A capo automatico, per attivare/disattivare rapidamente il wrapping delle righe senza aprire Opzioni.
• Aggiunte nel menu Modifica > Testo le nuove azioni per aumentare/ridurre il rientro, con scorciatoie Ctrl+Shift+. (indent) e Ctrl+Shift+, (de-indent), perché quando “Mostra voci nell’editor” è attivo il tasto Tab è riservato alla navigazione del pannello voci.
• Aggiunta la visualizzazione localizzata di data e ora per articoli RSS ed episodi podcast, con formato adattato alla lingua dell'interfaccia.
• Aggiunta nel menu contestuale RSS una nuova voce per condividere via email l'articolo selezionato.
• Aggiunte opzioni granulari di conferma eliminazione in Opzioni > RSS e podcast: per RSS (feed/articolo/entrambi/nessuno) e per Podcast (podcast/episodio/entrambi/nessuno).
• Aggiunta la copia rapida RSS configurabile con Ctrl+C (Opzioni > RSS e podcast): copia titolo, URL, contenuto articolo oppure tutto insieme.
• Unificato il flusso RSS: “Aggiungi Fonte” ora accetta sia URL feed sia parole chiave (con generazione automatica del feed Google News), senza necessità di una ricerca separata.
• Premendo Ctrl+A ora viene annunciato il completamento dell'azione per un feedback più chiaro con gli screen reader.
• Aggiunta la scorciatoia Shift+F3 per "Trova precedente" nel menu Modifica, in aggiunta a F3 "Trova successivo".
• Migliorato il messaggio di conferma delle sostituzioni con gestione corretta di singolare/plurale (es. “1 sostituzione” vs “2 sostituzioni”).
• Aggiunta nella finestra Dizionario la selezione della lingua di ricerca, con predefinito Auto (lingua interfaccia) e possibilità di override manuale.
• Aggiunta una nuova scheda Scorciatoie nelle Opzioni per personalizzare i tasti rapidi, con rilevamento dei conflitti e avviso quando una combinazione è già assegnata a un'altra azione.
• Aggiunto il supporto iniziale ai parametri da riga di comando: `-h`/`--help` mostrano la guida rapida e `--version` mostra la versione del programma.
• Resa più chiara la regolazione manuale di velocità e tono: i campi ora usano una scala centrata su 100, dove 100 corrisponde al valore normale.
• Migliorata la selezione delle voci Microsoft sia in Opzioni > Voce sia nel pannello voci dell’editor: aggiunta una casella combinata lingua localizzata per filtrare le voci per lingua, mantenendo la modalità “solo voci multilingua” come elenco unico non diviso per lingua (con combo lingua nascosta quando attiva).
• Aggiunta la configurazione della voce per i dialoghi in Opzioni > Voce con navigazione completa via Tab, usando lo stesso modello voci dell’interfaccia principale (sistema, filtro lingua Edge, voce e velocità/tono/volume con etichette); aggiunta anche la seconda voce dialoghi opzionale con gli stessi controlli (sistema, filtro lingua Edge, voce, velocità/tono/volume) per alternare i dialoghi; le regole dialoghi vengono salvate in configurazione `.ini`, senza modificare il testo del documento.
• Migliorata l’etichetta di Annulla: la voce Modifica > Annulla ora mostra l’azione che verrà annullata (ad esempio modifica testo, commenta/decommenta righe o inserimento tag voce), restando non disponibile quando non esiste nulla da annullare.
Correzioni di bug
• Corretto il supporto apertura RTF: i file `.rtf` ora vengono estratti e mostrati come testo leggibile, non più come markup RTF grezzo (es. `{\\rtf1...}`).
• Corretta l'apertura dei file di testo cinesi in codifica GB18030/GBK: Sonarpad ora li rileva e decodifica correttamente, evitando testo illeggibile (mojibake).
• Migliorata la creazione degli audiolibri M4B con metadata capitoli e marker capitolo; risolto il problema "chipmunk" (voce troppo veloce/acuta) nei file M4B generati.
• Corretta l'interfaccia bitrate nella finestra di salvataggio audiolibro: rimossi i testi hardcoded in italiano e aggiunta l'opzione 64 kbps tra i bitrate selezionabili.
• Corretto "Salva tutto" (Ctrl+Shift+S): ora tutti i documenti aperti modificati vengono rilevati in modo affidabile (inclusi tab nuovi/non salvati) e il salvataggio procede correttamente su ciascun file, aprendo "Salva con nome" quando necessario.
• Corretto l'ordinamento degli articoli RSS di Google News: quando la data è disponibile, gli articoli vengono ora mostrati dal più recente al meno recente.
• Corretta l'associazione etichette NVDA nella finestra Dizionario: campo ricerca e combobox lingua ora annunciano l'etichetta giusta.
• Corretta la gestione tastiera nella finestra Proprietà di RSS/Podcast: Tab/Shift+Tab raggiungono il pulsante OK, Invio attiva OK, Esc chiude in modo sicuro e il focus torna correttamente all'elenco RSS/Podcast.
• Corretto lo storico annullamento in RSS/Podcast: Ctrl+Z ora supporta annullamento multi-livello per rimozioni (articoli/episodi e fonti), non solo l'ultima azione.
• Migliorati gli annunci di rimozione in RSS/Podcast con messaggi espliciti (RSS rimosso, articolo RSS rimosso, episodio podcast rimosso).
• Migliorata la gestione del focus dopo elimina/annulla in RSS/Podcast: negli RSS viene selezionato in modo affidabile il primo feed quando necessario e sono state ridotte le ripetizioni degli annunci screen reader durante la riselezione ritardata.

Versione 0.6.6 – 2026-02-13
Miglioramenti
• Aggiunta "Formattazione automatica per TTS" nel menu Modifica per preparare rapidamente il testo alla lettura vocale (rimuove markdown/virgolette e ricompone le righe spezzate).
• Migliorato l'inserimento dei tag voce: ora, se è presente una selezione, i tag vengono applicati correttamente sia a una singola riga sia a più righe selezionate.
• Aggiunta un'opzione nelle impostazioni Audio per scegliere la cartella predefinita di salvataggio audiolibri (predefinita: Documenti\\Sonarpad Audiobooks).
• Nella finestra di salvataggio audiolibro, quando è attiva la divisione in parti, è stata aggiunta una nuova opzione (attiva di default) per creare una sottocartella dedicata alle parti generate.
• L'export audiolibri ora salva gli MP3 in stereo con bitrate scelto dall'utente per voci Edge, SAPI5 e SAPI4.
• Aggiunto supporto alle voci SAPI5 a 32 bit tramite bridge, così possono essere usate anche le voci disponibili solo nei motori a 32 bit.
• Riorganizzate le funzioni vocali in un menu dedicato "Voce e audio" e aggiunta/esplicitata la voce "Converti audio", utile per convertire qualunque file multimediale supportato in MP3, AAC, OGG, Opus, FLAC, WAV e AIFF.
• Aggiunta la rimozione dei singoli articoli RSS e dei singoli episodi podcast (tasto Canc + menu contestuale con conferma), senza eliminare l'intera fonte RSS/podcast, con annullamento dell'ultima rimozione (singolo articolo/episodio oppure intero podcast/feed RSS).
• Aggiunto l'export dei feed RSS in OPML nella finestra RSS, così le fonti correnti possono essere salvate e reimportate facilmente.
• Aggiunta la funzione "Cerca RSS per parola chiave" nella finestra RSS: inserendo una parola chiave viene generato automaticamente l'URL RSS di Google News e si apre la finestra di aggiunta fonte già precompilata, così i feed tematici si creano in un solo passaggio.
• Aggiunta la traduzione serba grazie a Mila Kuran.
• Aggiunta la traduzione ucraina grazie a Ivan Shtefuriak.
• Aggiunta l'apertura multipla dei file media: aprendo più file insieme viene creata una coda di riproduzione invece di sostituire il file corrente.
• Aggiunte scorciatoie di seek variabile durante la riproduzione: con base di 1 minuto, Freccia sinistra/destra sposta di 60s, Shift+Freccia sinistra/destra di 20s e Ctrl+Freccia sinistra/destra di 3 minuti.
• Aggiunte le scorciatoie per brano precedente/successivo nel player: Ctrl+Pagina su e Ctrl+Pagina giù.
• Aggiunta la voce "Reset volume" e raggruppate le azioni di ripristino in un sottomenu dedicato "Reset" in Riproduci, insieme a "Reset speed" e "Reset pitch".
• Migliorato l'installer: setup.exe ora permette di scegliere tra associare tutti i tipi file supportati oppure selezionare manualmente le singole estensioni; anche MSI ora espone la scelta per estensione nell'albero funzionalità (default invariato: tutte attive).
• Aggiunto il nuovo menu "Finestra" con la voce "Documenti aperti..." per passare rapidamente a uno dei file attualmente aperti.
• Aggiornata la voce Visualizza > Carattere: al posto del selettore completo ora c'è un sottomenu rapido con font comuni (Arial, Calibri, Consolas, Segoe UI, Tahoma, Verdana, Times New Roman, Georgia), mantenendo la dimensione testo già impostata.
• Migliorata la lettura di RSS e podcast con due annunci distinti: i nodi sorgente annunciano "nuovi elementi" quando il feed/podcast ha aggiornamenti, mentre i singoli articoli RSS e i singoli episodi podcast annunciano "non letto"/"non riprodotto"; il comportamento è disattivabile dalle Opzioni.
Correzioni di bug
• Corretto il parsing del testo EPUB per i libri che contengono commenti HTML inline (<!-- ... -->): il testo dei capitoli ora viene estratto correttamente invece di essere saltato in parte o del tutto.
• Corretto il dizionario Wiktionary in spagnolo e la gestione cache del dizionario: parole come "agua" ora vengono trovate correttamente e le vecchie cache "parola non trovata" non vengono più riutilizzate.
• Corretto l'encoding nell'import degli articoli RSS per alcune fonti spagnole (es. El Mundo): accenti e "ñ" ora vengono mantenuti correttamente nell'editor temporaneo.
• Corretta la decodifica ANSI dei file in lingue centro-europee (es. ceco/polacco): Sonarpad ora distingue meglio UTF-8 e ANSI e seleziona la code page corretta (inclusa Windows-1250), evitando diacritici corrotti.
• Corretta la persistenza delle fonti RSS con parametri nella URL (es. `rss.aspx?c=...`): questi feed ora vengono salvati e ripristinati correttamente dopo il riavvio di Sonarpad.
• Corretta l'apertura dei file puntatore Google Drive (`.gdoc`, `.gsheet`, `.gslides`) dal menu contestuale di Esplora file: se la lettura diretta fallisce con “Incorrect function (os error 1)”, Sonarpad ora usa un fallback shell-open e il documento si apre correttamente.
• Corretta la lettura dei file Excel legacy `.xls` (Excel 2010): ora i file binari vecchi vengono riconosciuti/decodificati correttamente invece di mostrare testo corrotto (es. `ÐÏ_à¡±...`).
• Corretto il flusso di annuncio del correttore ortografico: gli errori vengono ora riannunciati quando si rilegge il testo, e lo stesso errore viene segnalato di nuovo se viene cancellato e riscritto.
• Corrette le operazioni testuali a livello riga (es. Ctrl+Q / Ctrl+Shift+Q, ordina/inverti/righe uniche/unisci): selezionando una sola riga con Maiusc+Freccia giù non vengono più unite o troncate le righe adiacenti.
• Corretta la gestione delle selezioni multilinea nelle operazioni testuali a riga (Ctrl+Q / Ctrl+Shift+Q e strumenti correlati): quando RichEdit fornisce separatori di riga solo CR, il testo viene normalizzato correttamente e vengono elaborate tutte le righe selezionate senza tagli di caratteri.
• Estesa la normalizzazione input TTS per simboli visibili di spazi/tab/newline (␠/U+2420, ␣/U+2423, ␉/U+2409, ␊/U+240A, ␍/U+240D, ␤/U+2424), che con voci multilingua potevano causare ripetizioni dei paragrafi.
• Raffinata la sanitizzazione del testo Edge TTS con una pipeline unica di validazione: normalizzazione di spazi strani/invisibili, compattazione delle sequenze lunghe di punteggiatura (come "...", "!!!", "???") e salto dei chunk composti solo da punteggiatura per evitare loop di riproduzione.
• Corretto l'annuncio del tempo di riproduzione (Ctrl+I) per stream MP3/podcast: il tempo corrente ora viene limitato alla durata della traccia e la riproduzione viene fermata automaticamente se la posizione supera la fine.
• Migliorata la copertura di localizzazione dell'installer: setup.exe ora include anche ceco, polacco, francese e serbo, mentre l'MSI resta un unico pacchetto en-US per evitare confusione nelle release.
• Corretta la pulizia in disinstallazione delle voci del menu contestuale: "Apri con Sonarpad" ora viene rimosso in modo affidabile, anche in scenari legacy del registro.
• Corretta l'affidabilità di pausa/riprendi con SAPI5: la pausa con F4 ora funziona correttamente e la ripresa torna al punto previsto invece di ripartire dall'inizio.
• Corretto il flusso pausa + seek + riprendi nella riproduzione media: dopo pausa e spostamento con Freccia sinistra/destra, premendo Spazio la riproduzione riprende in modo affidabile dal punto corrente invece di fermarsi o ripartire dall'inizio.

Versione 0.6.5 – 2026-02-07
Miglioramenti
• Traduzione spagnola migliorata grazie ad Arturo Fernandez Rivas.
• Aggiornati i feed predefiniti: Affaritaliani, HuffPost Italia, La Gazzetta dello Sport. Rimosso Wired Italia.
• Aggiunta un'opzione per dividere gli audiolibri EPUB per capitoli.
• Ora la finestra per registrare i podcast è indipendente, in modo che possiate fare delle registrazioni e allo stesso tempo usare il programma Sonarpad!
• Gli articoli RSS ora usano una scheda temporanea dedicata (titolo localizzato); con Salva con nome diventa un documento normale.
• I messaggi dello screen reader ora vengono inviati anche a JAWS quando disponibile.
Correzioni di bug
• La lettura da cursore (F5) ora parte esattamente dal punto del cursore. Prima poteva partire alcune righe sopra perché l'offset del cursore non coincideva con le posizioni CRLF/UTF-16.
• Corretto un problema di redraw: digitando su una selezione il testo precedente poteva sparire finché non si spostava la selezione.
• Corretto il parsing dei capitoli EPUB: le pagine di copertina o solo immagini non generano più letture di CSS (es. "padding") o titoli "Sconosciuto".
• Corretto il problema degli audiolibri da EPUB con divisione per tempo: Edge TTS poteva fallire su chunk vuoti o troppo lunghi ("Edge audio not sent").
• Gli articoli RSS ora decodificano le entità HTML (es. &quot;, &amp;, &lt;, &gt;).
• Salva/Salva con nome ora propone il nome del file esistente quando si salvano formati non sovrascrivibili (es. EPUB), invece della prima riga.
• Risolto un problema per cui i podcast con nuovi episodi non venivano annunciati come non riprodotti, e rinominato "non ascoltato" in "non riprodotto" perché più professionale.

Versione 0.6.4 – 2026-02-05
Miglioramenti
• Il programma e' stato rinominato in Sonarpad per dare maggiore enfasi a suono e audio, che sono la chiave di questo programma.
• Aggiunta la selezione delle tracce audio nel menu Riproduzione per i file multimediali con più tracce audio (es. MKV con più lingue).
• I podcast ora indicano chiaramente quelli non ascoltati con il prefisso "Non ascoltato" prima del nome.
• Nuovo sistema di tag per cambiare voce nel testo. Esempi:
  - Voci Microsoft (Edge): <voice edge it-IT-IsabellaNeural>Ciao</voice>
  - Voci SAPI5: <voice sapi5 Microsoft Helena Desktop>Ciao</voice>
  - Voci SAPI4: <voice sapi4 #1>Ciao</voice>
  - Con velocita/tono/volume: <voice edge it-IT-ElsaNeural speed=-20 pitch=-5 volume=-10>Ciao</voice>
• Arricchite le categorie dei podcast.
• Migliorata la lettura dei PDF grazie al fallback automatico su PDFium.
• Migliorato il parser degli articoli che in alcuni casi non venivano letti in modo integrale.
• Aggiunto il reset del pitch nel menu Riproduci.
• Aggiunta un'opzione nel menu contestuale per creare un audiolibro dalla selezione.
• Aggiunta la divisione degli audiolibri in base alla durata, con la possibilita di scegliere il nome del primo file.
• Localizzata la voce che indica l'autore nella lettura degli articoli (es. "di", "by", "par").
• Aggiunte opzioni di indentazione (tab/spazi con larghezza) e Tab/Shift+Tab per indentare/deindentare le righe selezionate.
• Corretto il ripulisci Markdown: ora gestisce anche i bullet '*' quando non si mantengono le liste.
• Aggiunta un'opzione per usare il nome legacy "Novapad" nel titolo della finestra e nei collegamenti del menu Avvio.
Correzioni di bug
• Corretto un bug per cui gli audiolibri con SAPI4 potevano essere creati in modo diverso da quanto previsto.
• Corretto un bug per cui, andando oltre la fine con il seek, la riproduzione ripartiva dall'inizio.
• Finestra Trova nei file: premendo Invio su un risultato ora apre alla posizione corretta dello snippet e Esc torna ai risultati.
• Finestra Opzioni: sistemato il layout visivo delle schede Generale, Voce, Editor e Audio per evitare controlli mancanti o tagliati.
• Corretto un problema dei segnalibri quando si cambiava la velocità di riproduzione.
• Corretto un problema con Podcast Index e le categorie che non si visualizzavano correttamente.
• Corretto il problema dell'apostrofo che spezzava la lettura: ora non esiste più una lettura separata per i dialoghi, si usano i tag voce.

Versione 0.6.3 – 2026-01-30
Miglioramenti
• Migliorata la rilevazione del microfono.
• Aggiunta la riproduzione istantanea per tutti i formati.
Correzioni
• Corretto il crash nella finestra delle categorie podcast.

Versione 0.6.2 – 2026-01-30
Nuove funzionalità
• Aggiunta l'esecuzione dei file (Shift+F5). È possibile scegliere l'interprete (es. python) nelle Opzioni, cercarlo nel computer, e premendo Shift+F5 viene eseguito lo script corrente. I file HTML si aprono nel browser.
• Aggiunto il supporto per i file puntatori di Google Docs (.gdoc, .gsheet, .gslides), che si aprono automaticamente nel browser predefinito.
• Aggiunto il supporto per il formato audiolibro M4B (Apple/AAC).
• Aggiunta l'opzione "Mostra episodi" nel menu contestuale dei risultati di ricerca podcast per sfogliare e riprodurre episodi senza iscriversi.
• Aggiunta la funzione "Vai alla riga" (menu Modifica o Ctrl+J) per saltare rapidamente a un numero di riga specifico.
• Aggiunte opzioni nel menu contestuale per ordinare feed RSS e podcast (alfabeticamente o per data).
• Aggiunti feed RSS predefiniti in vietnamita.
• Aggiunta una casella di test microfono nella finestra di registrazione per verificare i livelli prima di iniziare.
• Aggiunta "Mostra descrizione" per gli episodi podcast nel menu contestuale.
• Aggiunto il supporto per formati audio/video estesi tramite FFmpeg: mkv, avi, mov, m4v, webm, mpg, ts, wmv, flv, vob, 3gp, flac, ogg, wma, aiff.
• Aggiunta la lettura sincronizzata dei sottotitoli (srt, vtt, ass, sub, sbv, lrc, smi) con NVDA o voce selezionata. Il programma cerca un file sottotitoli con lo stesso nome del file multimediale. Aggiunte le opzioni "Importa sottotitoli" e "Rimuovi sottotitoli" nel menu Riproduzione per file con nomi diversi.
• Aggiunte le associazioni file per tutti i nuovi formati audio/video supportati nel menu contestuale "Apri con Sonarpad".
• Aggiunta l'impostazione per regolare il pitch di qualsiasi file.
• Aggiunta nelle impostazioni Generali la casella per attivare o disattivare le segnalazioni di errore anonime. Aggiunta una voce nel menu Aiuto per creare un file ZIP diagnostico.
• Aggiunta l'opzione per usare una voce diversa per i dialoghi, sia per la lettura dal vivo che per la creazione di audiolibri.
• Aggiunto il browser delle categorie podcast per esplorare i podcast per categoria (business, arte, sport, ecc.).
Miglioramenti
• L'apertura di un file audio/video da Esplora risorse ora apre direttamente la vista player invece dell'editor di testo.
• Rimossa la richiesta OCR per i PDF non accessibili; l'OCR viene ora eseguito automaticamente per migliorare velocità ed esperienza utente.
• Migliorato il Terminale Accessibile: la lettura NVDA ora ricorda l'ultima riga letta per una migliore continuità.
• SAPI 4: La creazione di audiolibri è ora completamente parallelizzata e quasi istantanea. Aggiunta una richiesta per scegliere il numero di processi contemporanei.
• SAPI 4: Eliminato il collo di bottiglia WAV-MP3 convertendo i frammenti in parallelo durante la sintesi.
• SAPI 4: Migliorata la gestione degli errori e la pulizia automatica dei file temporanei.
• Finestra Trova: Rinominato "Regex" in "Espressione regolare" per chiarezza e aggiunte le traduzioni mancanti per le opzioni di ricerca.
• Audiolibri M4B: Migliore gestione dell'output; la divisione per parti/marcatori ora produce un singolo file M4B con metadati dei capitoli inclusi titolo e autore.
• Player: Corretta la precisione dei segnalibri e degli annunci del tempo quando la velocità di riproduzione non è 1.0x.
• Ripristinata la navigazione Ctrl+Tab e Ctrl+Shift+Tab nelle Opzioni.
• Aggiunta un'opzione nel menu Riproduzione per ripristinare istantaneamente la velocità Normale (1.0x).
• Aggiornate tutte le dipendenze alle ultime versioni per migliori prestazioni e stabilità.
• Integrato FFmpeg con caricamento dinamico delle DLL per garantire compatibilità senza bloccare l'avvio.
• Aggiornati i filtri di download podcast per includere i nuovi formati audio/video.
• Impedito a Ctrl+S di salvare file audio/video per evitare corruzione.
• Migliorata l'importazione delle trascrizioni YouTube rendendola più robusta e resiliente.
• Migliorata la robustezza della divisione in parti degli audiolibri, assicurando che nessun testo venga perso.
• L'installer è ora completamente multilingua, supportando Italiano, Inglese, Spagnolo, Portoghese, Svedese e Vietnamita in base alla lingua del sistema dell'utente. L'inglese è la lingua predefinita per i sistemi non supportati.
• Categorie podcast: premendo Invio su una categoria ora si conferma la selezione (equivalente al pulsante OK).
• Migliorato il sistema di rilevamento blocchi per evitare falsi positivi quando sono aperti dialoghi modali (messaggi di errore, "testo non trovato").
Correzioni
• Corretto un bug per cui il changelog non si apriva all'avvio.
• Corretto un bug per cui la richiesta OCR non appariva per i PDF non accessibili aperti da Esplora risorse.
• Corretto un bug all'avvio che poteva causare perdita di focus o chiusura delle finestre subito dopo l'apertura.
• Corretto un bug critico nella ricerca regex che impediva di trovare il testo, inclusi problemi con la "Ricerca circolare" e l'opzione "Il punto equivale a nuova riga" con le terminazioni di riga Windows.
Localizzazione
• Aggiunta la traduzione in polacco.
• Aggiunta la traduzione in francese.
• Aggiunta la traduzione in ceco (grazie a Radek Žalud e Jiri Holzinger).

Versione 0.6.1 – 2026-01-20
Correzioni
• Corretto un bug per cui, attivando “Visualizza le voci nell’editor” e riproducendo un podcast, la riproduzione veniva interrotta.
• Corretto un problema per cui alcuni podcast non potevano essere aggiunti tramite URL perché l’indirizzo veniva troncato.
• Corretto un bug per cui non era più possibile aggiungere URL normali nella funzione dei feed RSS.
• Corretto un problema per cui la lingua di Wikipedia veniva mostrata in più schede delle opzioni.
• Rimossa la creazione di alcuni file di debug che venivano generati anche in modalità release.
Miglioramenti
• Migliorato il supporto per le voci Microsoft, che ora vengono riprodotte utilizzando una modalità dedicata con un diverso user agent.
• Aggiunto il supporto per i file MP4.

Versione 0.6.0 – 2026-01-20
Nuove funzionalità
• Aggiunto il correttore ortografico. Dal menu contestuale è possibile verificare se la parola corrente è corretta e, in caso contrario, ottenere suggerimenti.
• Aggiunta l’importazione ed esportazione dei podcast tramite file OPML.
• Aggiunto il supporto alla ricerca Podcast Index oltre a iTunes. L’utente può inserire la propria API key e API secret gratuiti (generabili inserendo solo la propria email).
• Aggiunto il supporto alle voci SAPI4, sia per la lettura in tempo reale sia per la creazione di audiolibri
• Aggiunto il fallback automatico OCR per i PDF non accessibili: quando non viene trovato testo estraibile, il documento viene riconosciuto tramite OCR..
• Aggiunto il supporto al dizionario tramite Wiktionary. Premendo il tasto Applicazioni vengono mostrate le definizioni e, quando disponibili, anche sinonimi e traduzioni in altre lingue.
• Aggiunta l’importazione degli articoli da Wikipedia con ricerca, selezione dei risultati e importazione diretta nell’editor.
• Aggiunta la scorciatoia Shift+Invio nel modulo RSS per aprire un articolo direttamente nel sito web originale.
Miglioramenti
• La selezione del microfono ora viene sempre rispettata dall’applicazione.
• Nella finestra dei podcast, premendo Invio su un episodio NVDA annuncia immediatamente “caricamento”, dando subito conferma dell’operazione.
• Nei risultati di ricerca dei podcast, premendo Invio ora ci si sottoscrive al podcast selezionato.
• Corrette e migliorate le etichette delle scorciatoie Ctrl+Shift+O e Podcast Ctrl+Shift+P.
• La velocità di riproduzione e il volume ora vengono salvati nelle impostazioni e persistono per tutti i file audio.
• Aggiunta una cartella cache dedicata per gli episodi dei podcast. L’utente può conservare gli episodi tramite “Conserva podcast” nel menu Riproduci. La cache viene svuotata automaticamente quando supera la dimensione impostata dall’utente (Opzioni → Audio).
• Migliorato in modo significativo il recupero degli articoli RSS usando libcurl con impersonazione Chrome e iPhone, garantendo la compatibilità con circa il 99% dei siti.
• Aggiunto lo stato letto / non letto per gli articoli RSS, con indicazione chiara nella lista RSS.
• La funzione Sostituisci tutto ora mostra anche il numero di sostituzioni effettuate.
• Aggiunto il pulsante Elimina podcast quando si naviga la libreria dei podcast tramite Tab.
Correzioni
• Rimossa la voce ridondante “pending update” dal menu Aiuto (gli aggiornamenti sono già gestiti automaticamente).
• Corretto un bug per cui, aprendo un file MP3 e premendo Ctrl+S, il file veniva salvato e quindi corrotto.
• Corretto un problema nell’interfaccia in cui “Batch Audiobooks” veniva mostrato come “(B)… Ctrl+Shift+B” (rimossa l’etichetta ridondante).
• Corretto il funzionamento delle virgolette smart: quando abilitate, le virgolette normali vengono ora sostituite correttamente con quelle tipografiche.
• Corretto un bug per cui, usando “Vai al segnalibro”, la velocità di riproduzione veniva ripristinata a 1.0.
• Corretto un problema per cui gli episodi dei podcast già scaricati venivano riscaricati invece di usare la versione in cache.
Scorciatoie da tastiera
• F1 ora apre la guida.
• F2 ora controlla la presenza di aggiornamenti.
• F7 / F8 ora permettono di spostarsi all’errore ortografico precedente o successivo.
• F9 / F10 ora permettono di passare rapidamente tra le voci salvate nei preferiti.
Miglioramenti per sviluppatori
• Gli errori non vengono più ignorati silenziosamente: tutti i pattern let _ = sono stati rimossi e gli errori ora vengono gestiti esplicitamente (propagati, loggati o gestiti con fallback appropriati).
• Il progetto ora non compila in presenza di warning: sia cargo check sia cargo clippy devono completarsi senza avvisi, con lint più restrittivi e rimozione degli allow dove possibile.
• Rimosse le implementazioni personalizzate in stile strlen / wcslen. Le lunghezze delle stringhe e dei buffer UTF-16 ora derivano dai dati gestiti da Rust, senza scansioni manuali della memoria.
• La gestione delle DLL è stata ripulita e centralizzata attorno a libloading, evitando logiche di caricamento personalizzate e parsing PE.
• Rimossi gli helper artigianali per il parsing dei byte: ora tutto il parsing utilizza from_le_bytes / from_be_bytes su slice verificate.
Queste modifiche riducono l’uso superfluo di unsafe, eliminano potenziali comportamenti indefiniti e rendono il codice più idiomatico, robusto e manutenibile.

Versione 0.5.9 - 2026-01-13
Nuove funzionalita
• Aggiunta la possibilita di riordinare gli RSS dal menu contestuale (su/giu/posizione) con controlli per posizioni non valide.
• Aggiunto il menu contestuale anche per gli articoli, con apertura del sito originale e condivisione via WhatsApp, Facebook e X.
• Aggiunta la scorciatoia Esc per tornare rapidamente dagli articoli importati all'elenco RSS.
• Aggiunta la modalita podcast: ricerca, iscrizione e ascolto; riordinamento delle sottoscrizioni; Esc per fermare la riproduzione e tornare all'elenco; Invio su un episodio avvia la riproduzione.
• Aggiunta la regolazione della velocita di riproduzione per podcast e file MP3.
• Aggiunto Ctrl+T per andare a un tempo specifico.
• Aggiunto un pulsante di anteprima voci dopo la casella volume.
• Aggiunta la funzione regex per Trova e Sostituisci, stile Notepad++.
• Aggiunta l'importazione RSS da file OPML e TXT.
• Aggiunta nelle Opzioni la casella per abilitare "Apri con Sonarpad" in Esplora risorse, anche in versione portable.
• Aggiunto supporto OCR per PDF scansionati (richiede Windows 10/11): se un PDF non contiene testo, viene proposto il riconoscimento automatico.
Miglioramenti
• Migliorata la selezione di velocita, tono e volume delle voci, rispettando i limiti massimi del TTS.
• Vari miglioramenti alla modalita RSS per scaricare tutti gli articoli senza spostare il focus di NVDA durante gli aggiornamenti.
• Migliorata la riproduzione audio con un menu dedicato, annuncio tempo con Ctrl+I e volume fino al 300%.
• Aggiunte scorciatoie mancanti per alcune funzioni.
• Riordinato il menu Modifica con un sottomenu per le funzioni di pulizia testo.
• Riordinate le Opzioni in schede, con Ctrl+Tab e Ctrl+Shift+Tab per spostarsi tra le schede.
• Risolti i problemi di lettura degli articoli: il lettore RSS ora legge integralmente gli articoli come da browser.
Fix
• Corretto un problema per cui la pulizia Markdown eliminava i numeri a inizio riga.
• Corretto il problema AltGr+Z che attivava Undo.
• Corretto un problema per cui la registrazione di un audiolibro non si poteva interrompere rapidamente.
Localizzazione
• Aggiunta la traduzione vietnamita (grazie a Anh Đức Nguyễn).

Versione 0.5.8 - 2026-01-10
Nuove funzionalita
• Aggiunto il controllo volume per microfono e audio di sistema durante la registrazione podcast.
• Aggiunta una nuova funzione per importare articoli da siti web o feed RSS, includendo per ogni lingua i feed piu importanti.
• Aggiunta la funzione per rimuovere tutti i segnalibri del file corrente.
• Aggiunta la funzione per rimuovere le linee duplicate e le linee duplicate consecutive.
• Aggiunta la funzione per chiudere tutti i tab o le finestre tranne quella corrente.
• Inserita la voce Donazioni nel menu Aiuto per tutte le lingue.
Miglioramenti
• Migliorato il terminale accessibile evitando alcuni crash.
• Migliorati e sistemati access key e scorciatoie da tastiera del programma.
• Corretto un problema per cui chiudendo la finestra di riproduzione audio la riproduzione non si fermava.
• Aggiunte finestre di conferma per azioni importanti (es. rimozione linee duplicate, rimozione trattini a fine riga, rimozione di tutti i segnalibri del file corrente). Nessuna conferma se l'azione non si applica.
• Aggiunta la possibilita di eliminare feed/siti RSS dalla libreria selezionandoli e premendo Canc.
• Aggiunto un menu contestuale nella finestra RSS per modificare o eliminare feed/siti RSS.
• Rimossa la casella per spostare le impostazioni nella cartella corrente: ora il programma lo gestisce automaticamente (se la cartella dell'exe si chiama "sonarpad portable" o l'exe e su un drive rimovibile, salva nella cartella dell'exe in `config`, altrimenti in `%APPDATA%\\Sonarpad`, con fallback a `config` se la cartella preferita non e scrivibile).

Versione 0.5.7 - 2026-01-05
Nuove funzionalita
• Aggiunta l'opzione per registrare audiolibri in batch (conversione multipla di file e cartelle).
• Aggiunto il supporto per i file Markdown (.md).
• Aggiunta la scelta della codifica (encoding) all'apertura dei file di testo.
• Aggiunta l'opzione nel terminale per annunciare con NVDA le nuove righe in arrivo.
Miglioramenti
• Il salvataggio delle registrazioni (audiolibri) avviene ora in MP3 nativo quando selezionato.
• L'utente può scegliere dove inserire l'asterisco * che indica le modifiche non salvate (titolo finestra).
• Migliorato il sistema di aggiornamento per renderlo più robusto in diversi scenari.
• Aggiunta nel menu Modifica la funzione per rimuovere i trattini a fine riga (utile per testi OCR).

Versione 0.5.6 - 2026-01-04
Fix
  Migliorata Trova nei file: premendo Invio apre il file esattamente alla posizione dello snippet selezionato.
Miglioramenti
  Aggiunto supporto PPT/PPTX.
  Per i formati non testuali, Salva ora propone sempre .txt per evitare di rovinare la formattazione (PDF/DOC/DOCX/EPUB/HTML/PPT/PPTX).
  Aggiunta registrazione podcast da microfono e audio di sistema (menu File, Ctrl+Shift+R).

Versione 0.5.5 - 2026-01-03
Nuove funzionalita
• Aggiunto un terminale accessibile ottimizzato per programmi che inviano molto output agli screen reader (Ctrl+Shift+P).
• Aggiunta l'opzione per salvare le impostazioni utente nella cartella corrente (modalita' portable).
Fix
• Migliorati gli snippet di Trova nei file per mantenere l'anteprima allineata alla corrispondenza.

Versione 0.5.4 – 2026-01-03
Miglioramenti
• Fix alla funzione Normalizza spazi bianchi (Ctrl+Shift+Invio).
• Aggiunto supporto HTML/HTM (apertura come testo).

Versione 0.5.3 – 2026-01-02
Nuove funzionalita
• Aggiunto Trova nei file.
• Aggiunti nuovi strumenti di testo: Normalizza spazi bianchi, Riformatta righe e Pulisci testo Markdown.
• Aggiunte Statistiche testo (Alt+Y).
• Aggiunti nuovi comandi lista nel menu Modifica:
• Ordina righe (Alt+Shift+O)
• Rimuovi duplicati (Alt+Shift+K)
• Inverti righe (Alt+Shift+Z)
• Aggiunti Commenta / Decommenta righe (Ctrl+Q / Ctrl+Shift+Q).
Localizzazione
• Aggiunta la lingua spagnola.
• Aggiunta la lingua portoghese.
Miglioramenti
• Quando un file EPUB e' aperto, Salva passa automaticamente a Salva con nome ed esporta il contenuto come .txt per evitare corruzione dell'EPUB.

## 0.5.2 - 2026-01-01

* Aggiunto il changelog.
* Aggiunte le opzioni "Apri con Sonarpad" e le associazioni per i file supportati durante l'installazione.
* Migliorata la localizzazione dei messaggi (errori, dialoghi, esportazione audiolibro).
* Aggiunta la selezione delle parti quando si usa "Dividi l'audiolibro in base al testo", con opzione "Il testo deve iniziare a capo".
* Aggiunta l'importazione trascrizioni da YouTube con selezione lingua, opzione timestamp e gestione focus.

## 0.5.1 - 2025-12-31

* Aggiornamento automatico con conferma, gestione errori e notifiche migliorate.
* Esportazione audiolibro migliorata (split per testo, SAPI5/Media Foundation, controlli avanzati).
* Miglioramenti TTS (pausa/riprendi, dizionario sostituzioni, preferiti).
* Menu Vista e pannelli voci/favoriti, colore e dimensione testo.
* Lingua predefinita dal sistema e miglioramenti localizzazione.
* CI e packaging Windows (artefatti, MSI/NSIS, cache).

## 0.5.0 - 2025-12-27

* Refactor modulare (editor, file handler, menu, ricerca).
* Workflow di build/packaging Windows e aggiornamenti README/licenza.
* Fix navigazione TAB in finestra Guida.

## 0.5 - 2025-12-27

* Aggiornamento numero versione preliminare.

## 0.1.0 - 2025-12-25

* Prima versione: struttura progetto e README iniziale.

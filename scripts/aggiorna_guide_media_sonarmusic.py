"""Aggiorna le 18 guide Sonarpad con Media Cutter e SonarMusic.

Idempotente; preserva il testo preesistente e la convenzione dei fine riga.
Eseguire dalla radice del progetto con Python 3.
"""
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parent.parent
MARKER = '========== MEDIA CUTTER / SONARMUSIC =========='
SECTIONS = {
'': '''TAGLIA FILE MEDIA — GUIDA COMPLETA

Per aprire la funzione, vai in Strumenti, Multimedia, Taglia file media oppure premi Alt+Maiusc+X. Seleziona Apri file media e scegli un file audio o video locale. Questa funzione serve a eliminare intervalli indesiderati e salvare un nuovo file con le parti conservate; è diversa da Dividi media, che spezza un file in più file.

In Taglio guidato scegli quanto spostarti con i comandi Indietro e Avanti: 1, 5, 10, 30 o 60 secondi. Puoi ascoltare con Ctrl+Spazio e spostarti con Alt+Freccia sinistra/destra. Raggiungi l'inizio del tratto da togliere e premi Inizio taglio (Alt+1). Vai alla fine del tratto e premi Fine taglio (Alt+2). Premi Ascolta taglio (Alt+3) per verificare la parte che verrà rimossa, poi Applica taglio (Alt+4). Ripeti per altri intervalli. Non è necessario conoscere il video visivamente: il programma comunica posizioni e azioni tramite messaggi accessibili.

L'Area comandi è pensata per NVDA e JAWS: spostati su questo controllo con Tab. Al suo interno funzionano direttamente Freccia sinistra/destra, 1 (inizio), 2 (fine), 3 o Invio (ascolta), 4 (applica), Spazio (riproduci/pausa), Home e Fine. Maiusc+Frecce orizzontali salta di 5 secondi, Ctrl+Frecce orizzontali di 10. F1 legge i comandi disponibili per la modalità corrente; Tab esce dall'area. Le combinazioni Alt restano utilizzabili anche fuori dall'Area comandi.

In Taglio avanzato puoi suddividere il file in parti con S (Dividi qui), muoverti tra le parti con Freccia su/giù nell'Area comandi, ascoltare la parte selezionata con 3 o Invio, eliminarla con D o Canc, ripristinarla con R e inserire un altro file media prima della parte selezionata. Puoi correggere gli estremi usando i campi Inizio e Fine e la precisione di 0,01, 0,1, 0,5 o 1 secondo. Se il file contiene video puoi anche scegliere la rotazione e mostrare un'anteprima. È possibile aggiungere una traccia audio, regolare separatamente i volumi dell'originale e della traccia aggiunta e ripetere quest'ultima se è troppo corta.

Alla fine premi Salva o Ctrl+S, scegli percorso e formato di uscita e assegna al nuovo file un nome diverso da quello originale. Durante l'esportazione sono visualizzati avanzamento e pulsante Interrompi. Se c'è un taglio guidato ancora in sospeso, completalo prima di salvare. Quando chiudi con modifiche non salvate, Sonarpad chiede conferma.

SONARMUSIC — GUIDA COMPLETA

Apri SonarMusic da Strumenti, Multimedia, SonarMusic oppure con Ctrl+Alt+Maiusc+M. Inserisci il titolo di una canzone, il nome di un artista o un album nel campo di ricerca. Seleziona Tipo ricerca: Tutto, Brani, Album, Artisti, Playlist o Video musicali. Premi Invio nel campo di ricerca o il pulsante Cerca. Il filtro scelto viene ricordato. Il pulsante Suggerimenti, quando disponibile, aiuta a completare la ricerca.

Esplora la casella Risultati ricerca con le frecce. Premi Invio sull'elemento scelto: un brano o video riproducibile viene aperto nel lettore; un album, artista o playlist apre i relativi contenuti. Le voci per passare ai risultati precedenti o caricarne altri sono presenti direttamente nell'elenco. Dopo il caricamento il focus torna ai risultati; quando si termina la riproduzione vengono recuperati, ove possibile, ricerca e selezione precedenti.

Per le altre azioni seleziona un risultato e premi il tasto Applicazioni o Maiusc+F10 per aprire il menu contestuale. A seconda del tipo di elemento troverai Apri, Salva media, Trascrivi, Crea audiodescrizione con IA e Aggiungi o Rimuovi dai preferiti. Alcune azioni compaiono solo per brani o video riproducibili; album, artisti e playlist possono essere aperti per esplorarli. Per salvare un media segui le finestre e le opzioni disponibili, rispettando eventuali limitazioni del servizio.

La casella Preferiti permette di recuperare gli elementi memorizzati: scegli un elemento e premi Invio per aprirlo. Con il tasto Applicazioni o Maiusc+F10 puoi anche rimuoverlo. Il pulsante Indietro torna alla ricerca o alla pagina precedente; Esc torna indietro quando possibile, oppure chiude SonarMusic. Tutta la finestra può essere percorsa con Tab, Maiusc+Tab e le frecce, usando NVDA o JAWS. Le ricerche musicali richiedono una connessione Internet.
''',
'_en': '''CUT MEDIA FILE — COMPLETE GUIDE

Open Tools, Multimedia, Cut media file, or press Alt+Shift+X. Choose Open media file and select a local audio or video. This feature removes unwanted passages and exports a new file containing the retained material. It differs from Split media, which produces several separate files.

In Guided cut mode, select a movement step of 1, 5, 10, 30 or 60 seconds. Use Ctrl+Space to play or pause and Alt+Left/Right to seek. Move to the beginning of the unwanted passage and select Start cut (Alt+1); move to its end and select End cut (Alt+2). Use Listen to cut (Alt+3) to review what will be removed, then Apply cut (Alt+4). Repeat for additional cuts.

The Command area is designed for keyboard and screen-reader use. Tab to it and use Left/Right to seek, 1 for start, 2 for end, 3 or Enter to listen, 4 to apply, Space to play/pause, and Home/End for the start/end of the file. Shift+Left/Right seeks five seconds and Ctrl+Left/Right ten seconds. F1 announces the commands available in the current mode. Tab leaves the area; Alt shortcuts also work outside it.

In Advanced cut mode, press S to split at the current point, Up/Down in the Command area to select a segment, 3 or Enter to listen, D or Delete to remove, and R to restore. You may insert another media file before the selected segment and precisely adjust start/end values in steps of 0.01, 0.1, 0.5 or 1 second. Video options include rotation and preview. You may also add an audio track, set separate original and added-track volumes and loop the added track if needed.

Press Save or Ctrl+S, choose an output format and location, and use a filename different from the original. Export has progress feedback and a Cancel button. Complete a pending guided cut before saving; closing unsaved changes prompts for confirmation.

SONARMUSIC — COMPLETE GUIDE

Open Tools, Multimedia, SonarMusic, or press Ctrl+Alt+Shift+M. Enter a song title, artist or album in the search field. Choose a Search type: All, Songs, Albums, Artists, Playlists or Music videos. Press Enter in the search field or choose Search. The selected filter is remembered. Suggestions may be offered while entering text.

Use the arrow keys in Search results and press Enter on an item. Playable songs and videos open in the player; albums, artists and playlists open their contents. The results list also includes entries for previous or additional results. On loading, focus returns to the results; after playback, the search and selection are restored where possible.

Select an item and press the Applications key or Shift+F10 to open its context menu. Depending on the item, actions include Open, Save/download media, Transcribe, Create AI audio description and Add to or Remove from favorites. Playback-specific actions are offered only for playable items. Follow the available dialogs and service restrictions when saving media.

Use the Favorites list to find saved items. Choose a favorite and press Enter to open it; use the Applications key or Shift+F10 to remove it. Back returns to a previous search or page; Escape goes back when possible or closes the window. Tab, Shift+Tab and arrow keys support NVDA and JAWS navigation. Music searches require an Internet connection.
''',
'_cs': '''STŘIH MULTIMEDIÁLNÍHO SOUBORU — PODROBNÝ NÁVOD

Otevřete Nástroje, Multimédia, Střih multimediálního souboru nebo stiskněte Alt+Shift+X. Tlačítkem Otevřít multimediální soubor vyberte místní zvukový soubor či video. Funkce odstraní nežádoucí úseky a uloží nový soubor se zachovaným obsahem. Nejde o rozdělení na několik samostatných souborů.

V režimu průvodce nastavte krok posunu na 1, 5, 10, 30 nebo 60 sekund. Ctrl+Mezerník spouští a pozastavuje přehrávání, Alt+šipka vlevo/vpravo posouvá. Na začátku úseku určeného k odstranění použijte Alt+1, na jeho konci Alt+2. Pomocí Alt+3 si poslechněte označený úsek a Alt+4 jej odstraňte. Postup lze opakovat.

Pro práci s odečítačem obrazovky přejděte klávesou Tab do oblasti příkazů. Zde šipky vlevo/vpravo posouvají, 1 označuje začátek, 2 konec, 3 nebo Enter spustí poslech, 4 provede střih, mezerník přehrává či pozastavuje a Home/End přecházejí na začátek/konec souboru. Shift+šipky posouvají o 5 sekund, Ctrl+šipky o 10 sekund. F1 přečte dostupné příkazy; Tab oblast opustí.

V pokročilém režimu lze klávesou S rozdělit úsek, šipkami nahoru/dolů vybírat části, klávesou D nebo Delete část odstranit a R ji obnovit. Je možné vložit další médium před vybranou část, přesně upravit časy začátku a konce po 0,01 až 1 sekundě, u videa změnit otočení a zobrazit náhled. Lze také přidat zvukovou stopu, nastavit hlasitost obou stop a opakování přidané stopy.

Uložte výsledek tlačítkem Uložit nebo Ctrl+S. Vyberte jiný název než u původního souboru. Export zobrazuje průběh a nabízí zrušení; před uložením dokončete rozpracovaný střih. Při zavírání neuložených změn se zobrazí potvrzení.

SONARMUSIC — PODROBNÝ NÁVOD

V nabídce Nástroje, Multimédia otevřete SonarMusic nebo použijte Ctrl+Alt+Shift+M. Zadejte skladbu, interpreta či album. Filtr vyhledávání nabízí Vše, Skladby, Alba, Umělce, Playlisty a Hudební videa. Stiskněte Enter v poli vyhledávání nebo tlačítko Hledat. Volba filtru se ukládá a mohou se nabídnout návrhy hledání.

V seznamu výsledků se pohybujte šipkami a potvrďte Enterem. Skladba či video se otevře v přehrávači; album, interpret nebo playlist zobrazí obsah. Položky pro předchozí a další výsledky jsou součástí seznamu. Po načtení se kurzor vrací do výsledků a po přehrávání se pokud možno obnoví výběr.

Klávesou kontextové nabídky nebo Shift+F10 otevřete akce vybrané položky: Otevřít, Uložit médium, Přepsat, Vytvořit audiopopis pomocí AI nebo Přidat/Odebrat z oblíbených. Nabídka závisí na druhu výsledku; ukládání podléhá dostupným možnostem a omezením služby.

Uložené položky najdete v seznamu Oblíbené: Enter je otevře, kontextová nabídka je umožní odebrat. Zpět se vrací na předchozí stránku, Escape se vrací nebo zavírá okno. Tab, Shift+Tab a šipky umožňují ovládání s NVDA nebo JAWS. Vyhledávání vyžaduje internet.
''',
'_de': '''MEDIENDATEI SCHNEIDEN — AUSFÜHRLICHE ANLEITUNG

Öffnen Sie Werkzeuge, Multimedia, Mediendatei schneiden oder drücken Sie Alt+Umschalt+X. Wählen Sie Mediendatei öffnen und anschließend eine lokale Audio- oder Videodatei. Die Funktion entfernt unerwünschte Abschnitte und speichert die verbleibenden Teile als neue Datei. Sie unterscheidet sich von der Aufteilung einer Datei in mehrere Dateien.

Im geführten Schnitt wählen Sie eine Schrittweite von 1, 5, 10, 30 oder 60 Sekunden. Mit Strg+Leertaste starten oder pausieren Sie die Wiedergabe, mit Alt+Pfeil links/rechts bewegen Sie sich. Markieren Sie den Beginn des zu entfernenden Abschnitts mit Alt+1 und sein Ende mit Alt+2. Alt+3 spielt den markierten Abschnitt zur Kontrolle ab; Alt+4 wendet den Schnitt an. Weitere Abschnitte können anschließend bearbeitet werden.

Für Screenreader gibt es den Befehlsbereich, den Sie mit Tab erreichen. Darin bewegen die Pfeiltasten links/rechts die Position; 1 setzt den Anfang, 2 das Ende, 3 oder Eingabe spielt den Abschnitt ab, 4 wendet den Schnitt an, Leertaste startet/pausiert und Pos1/Ende springen zu Dateianfang/-ende. Umschalt+Pfeil springt 5 Sekunden, Strg+Pfeil 10 Sekunden. F1 liest die verfügbaren Befehle vor; mit Tab verlassen Sie den Bereich. Die Alt-Kombinationen funktionieren auch außerhalb.

Im erweiterten Modus teilen Sie mit S an der aktuellen Position, wechseln im Befehlsbereich mit Pfeil hoch/runter zwischen Teilen, löschen mit D oder Entf und stellen mit R wieder her. Sie können vor einem ausgewählten Teil eine weitere Mediendatei einfügen, Anfangs- und Endzeit in Schritten von 0,01 bis 1 Sekunde einstellen, Videos drehen und eine Vorschau anzeigen. Eine zusätzliche Audiospur mit getrennten Lautstärken und optionaler Wiederholung wird ebenfalls unterstützt.

Mit Speichern oder Strg+S wählen Sie Speicherort und Ausgabeformat. Verwenden Sie einen anderen Dateinamen als beim Original. Beim Export gibt es eine Fortschrittsanzeige und Abbrechen. Beenden Sie einen begonnenen geführten Schnitt vor dem Speichern; ungespeicherte Änderungen werden beim Schließen bestätigt.

SONARMUSIC — AUSFÜHRLICHE ANLEITUNG

Öffnen Sie Werkzeuge, Multimedia, SonarMusic oder drücken Sie Strg+Alt+Umschalt+M. Geben Sie einen Songtitel, Interpreten oder ein Album ein. Als Suchtyp stehen Alles, Songs, Alben, Künstler, Playlists und Musikvideos zur Auswahl. Drücken Sie im Suchfeld Eingabe oder wählen Sie Suchen. Der gewählte Filter wird gespeichert; Suchvorschläge können eingeblendet werden.

Navigieren Sie mit den Pfeiltasten durch die Suchergebnisse und drücken Sie Eingabe. Abspielbare Titel und Videos öffnen sich im Player, Alben, Künstler und Playlists zeigen ihre Inhalte. Im Ergebnisfeld gibt es auch Einträge für vorherige oder weitere Ergebnisse. Nach dem Laden wird der Fokus auf die Ergebnisse gesetzt; nach der Wiedergabe werden Suche und Auswahl möglichst wiederhergestellt.

Über die Anwendungstaste oder Umschalt+F10 öffnen Sie das Kontextmenü. Je nach Eintrag stehen Öffnen, Medium speichern/herunterladen, Transkribieren, KI-Audiodeskription erstellen sowie Zu Favoriten hinzufügen/Aus Favoriten entfernen bereit. Nicht jede Aktion ist für jeden Eintrag verfügbar. Beim Speichern gelten die verfügbaren Dialoge und Einschränkungen des Dienstes.

Über die Favoritenliste öffnen Sie gespeicherte Elemente mit Eingabe und entfernen sie über das Kontextmenü. Zurück führt zur vorherigen Ansicht, Escape geht zurück oder schließt das Fenster. Tab, Umschalt+Tab und Pfeiltasten erleichtern die Bedienung mit NVDA und JAWS. Für die Suche ist Internet erforderlich.
''',
'_es': '''CORTAR ARCHIVO MULTIMEDIA — GUÍA COMPLETA

Abre Herramientas, Multimedia, Cortar archivo multimedia, o pulsa Alt+Mayús+X. Selecciona Abrir archivo multimedia y elige un audio o vídeo local. Esta función elimina fragmentos no deseados y guarda un archivo nuevo con las partes conservadas; no es lo mismo que dividir un archivo en varios archivos separados.

En Corte guiado, elige un salto de 1, 5, 10, 30 o 60 segundos. Ctrl+Espacio inicia o pausa la reproducción; Alt+Flecha izquierda/derecha permite desplazarse. Sitúate al principio del fragmento que quieres quitar y pulsa Alt+1; ve al final y pulsa Alt+2. Usa Alt+3 para escuchar el fragmento marcado y Alt+4 para aplicar el corte. Repite los pasos para otros fragmentos.

El Área de comandos está pensada para lectores de pantalla. Llega con Tab; dentro, Izquierda/Derecha desplazan, 1 marca el inicio, 2 el final, 3 o Intro escucha, 4 aplica, Espacio reproduce/pausa e Inicio/Fin saltan a los extremos del archivo. Mayús+Flechas salta 5 segundos y Ctrl+Flechas 10 segundos. F1 anuncia los comandos disponibles; Tab sale del área. Los atajos con Alt también funcionan fuera del área.

En Corte avanzado, S divide en la posición actual; Arriba/Abajo seleccionan partes, D o Supr elimina una parte y R la restaura. Puedes insertar otro archivo multimedia antes de la parte seleccionada, ajustar inicio y fin con pasos de 0,01, 0,1, 0,5 o 1 segundo, girar vídeos y mostrar su vista previa. También puedes añadir una pista de audio, ajustar el volumen de ambas pistas y repetir la nueva pista si se acaba antes.

Pulsa Guardar o Ctrl+S, elige formato y destino y utiliza un nombre diferente del original. La exportación muestra el progreso y ofrece Cancelar. Termina cualquier corte guiado pendiente antes de guardar; al cerrar con cambios sin guardar se solicita confirmación.

SONARMUSIC — GUÍA COMPLETA

Abre Herramientas, Multimedia, SonarMusic, o pulsa Ctrl+Alt+Mayús+M. Escribe una canción, un artista o un álbum en el campo de búsqueda. El filtro Tipo de búsqueda permite elegir Todo, Canciones, Álbumes, Artistas, Listas y Vídeos musicales. Pulsa Intro en el campo o el botón Buscar. Se recuerda el filtro seleccionado y pueden aparecer sugerencias.

Usa las flechas en Resultados de búsqueda y pulsa Intro sobre un elemento. Las canciones y los vídeos reproducibles se abren en el reproductor; los álbumes, artistas y listas muestran su contenido. El listado incluye opciones para ir a resultados anteriores o cargar más. El foco vuelve a los resultados después de cargarlos y, cuando es posible, se restaura la búsqueda tras la reproducción.

Con la tecla Aplicaciones o Mayús+F10 abre el menú contextual del elemento elegido. Según el tipo, ofrece Abrir, Guardar/descargar, Transcribir, Crear audiodescripción con IA y Añadir o Quitar de favoritos. Algunas acciones solo están disponibles para medios reproducibles. Para guardar sigue los diálogos y restricciones del servicio.

En Favoritos, selecciona un elemento y pulsa Intro para abrirlo; desde el menú contextual puedes quitarlo. Atrás vuelve a páginas anteriores; Escape vuelve atrás cuando es posible o cierra la ventana. Tab, Mayús+Tab y las flechas permiten utilizar la función con NVDA o JAWS. La búsqueda necesita Internet.
''',
'_fr': '''DÉCOUPER UN FICHIER MULTIMÉDIA — GUIDE COMPLET

Ouvrez Outils, Multimédia, Découper un fichier multimédia, ou appuyez sur Alt+Maj+X. Choisissez Ouvrir un fichier multimédia puis un fichier audio ou vidéo local. La fonction supprime les passages indésirables et exporte un nouveau fichier contenant les passages conservés. Elle est distincte du partage d'un fichier en plusieurs fichiers.

En découpage guidé, choisissez un déplacement de 1, 5, 10, 30 ou 60 secondes. Ctrl+Espace lance ou met la lecture en pause et Alt+Flèche gauche/droite déplace la position. Au début du passage à supprimer, appuyez sur Alt+1 ; à la fin, sur Alt+2. Alt+3 permet d'écouter la portion sélectionnée et Alt+4 applique la coupe. Vous pouvez recommencer pour d'autres passages.

La Zone de commandes est adaptée aux lecteurs d'écran. Rejoignez-la avec Tab. Les flèches gauche/droite naviguent, 1 marque le début, 2 la fin, 3 ou Entrée écoute, 4 applique la coupe, Espace lance/met en pause, Origine/Fin rejoignent les extrémités du fichier. Maj+Flèches avance ou recule de 5 secondes et Ctrl+Flèches de 10 secondes. F1 annonce les commandes disponibles ; Tab quitte la zone. Les raccourcis Alt fonctionnent aussi en dehors.

En découpage avancé, S coupe à la position actuelle, Haut/Bas sélectionnent les segments, D ou Suppr supprime un segment et R le restaure. Vous pouvez insérer un autre média avant le segment choisi, ajuster précisément les bornes par pas de 0,01 à 1 seconde, tourner une vidéo et afficher un aperçu. Vous pouvez aussi ajouter une piste audio, régler séparément les volumes et mettre la piste ajoutée en boucle.

Cliquez sur Enregistrer ou Ctrl+S, choisissez le format et l'emplacement et donnez au nouveau fichier un nom différent de l'original. Une barre de progression et un bouton Annuler sont disponibles pendant l'exportation. Terminez toute coupe guidée en attente avant d'enregistrer ; la fermeture avec des modifications non enregistrées demande confirmation.

SONARMUSIC — GUIDE COMPLET

Ouvrez Outils, Multimédia, SonarMusic ou appuyez sur Ctrl+Alt+Maj+M. Saisissez une chanson, un artiste ou un album. Le filtre Type de recherche propose Tout, Titres, Albums, Artistes, Playlists et Vidéos musicales. Validez avec Entrée dans la recherche ou le bouton Rechercher. Le filtre est mémorisé ; des suggestions peuvent apparaître.

Parcourez les résultats avec les flèches puis Entrée. Un titre ou une vidéo lisible s'ouvre dans le lecteur ; un album, un artiste ou une playlist affiche son contenu. La liste contient aussi les commandes pour les résultats précédents ou suivants. Après chargement, le focus revient aux résultats ; la recherche et la sélection sont rétablies après lecture lorsque c'est possible.

La touche Applications ou Maj+F10 ouvre le menu contextuel : Ouvrir, Enregistrer/télécharger, Transcrire, Créer une audiodescription par IA, Ajouter ou Retirer des favoris, selon le type d'élément. Certaines actions sont réservées aux médias lisibles. Respectez les options disponibles et les restrictions du service lors de l'enregistrement.

La liste Favoris permet de retrouver les éléments enregistrés : Entrée ouvre un élément, le menu contextuel permet de le retirer. Retour revient à la page précédente ; Échap revient en arrière ou ferme la fenêtre. Tab, Maj+Tab et les flèches facilitent la navigation avec NVDA ou JAWS. La recherche nécessite Internet.
''',
'_hi': '''मीडिया फ़ाइल काटना — विस्तृत मार्गदर्शिका

टूल्स, मल्टीमीडिया, मीडिया फ़ाइल काटें विकल्प खोलें या Alt+Shift+X दबाएँ। मीडिया फ़ाइल खोलें चुनकर अपने कंप्यूटर की ऑडियो या वीडियो फ़ाइल चुनें। इस सुविधा से अनचाहे हिस्से हटाकर शेष सामग्री की नई फ़ाइल सहेजी जाती है। यह फ़ाइल को अलग-अलग कई फ़ाइलों में बाँटने से अलग है।

निर्देशित कट मोड में 1, 5, 10, 30 या 60 सेकंड का कदम चुनें। Ctrl+Space से चलाएँ या रोकें और Alt+बायाँ/दायाँ तीर से स्थान बदलें। हटाए जाने वाले हिस्से की शुरुआत पर Alt+1 और अंत पर Alt+2 दबाएँ। Alt+3 से चयनित हिस्सा सुनें और Alt+4 से कट लागू करें। अन्य हिस्सों के लिए प्रक्रिया दोहराएँ।

स्क्रीन रीडर के लिए कमांड क्षेत्र में Tab से जाएँ। वहाँ बाएँ/दाएँ तीर से आगे-पीछे जाएँ, 1 से शुरुआत, 2 से अंत, 3 या Enter से सुनें, 4 से काटें, Space से चलाएँ/रोकें और Home/End से फ़ाइल के आरंभ/अंत पर जाएँ। Shift+तीर से 5 सेकंड और Ctrl+तीर से 10 सेकंड का परिवर्तन होता है। F1 उपलब्ध आदेश पढ़कर सुनाता है; Tab क्षेत्र से बाहर निकलता है। Alt वाले शॉर्टकट बाहर भी चलते हैं।

उन्नत कट मोड में S से भाग विभाजित करें, ऊपर/नीचे तीर से भाग चुनें, D या Delete से हटाएँ और R से वापस लाएँ। चयनित भाग से पहले दूसरी मीडिया फ़ाइल जोड़ी जा सकती है। आरंभ और अंत 0.01 से 1 सेकंड के कदमों में बदले जा सकते हैं। वीडियो घुमाना और पूर्वावलोकन देखना भी संभव है। नई ऑडियो ट्रैक जोड़कर दोनों ट्रैक की आवाज़ अलग-अलग नियंत्रित और नई ट्रैक को दोहराया जा सकता है।

Save या Ctrl+S दबाकर आउटपुट का प्रारूप और स्थान चुनें। मूल फ़ाइल से अलग नाम दें। निर्यात की प्रगति और रोकने का विकल्प दिखाई देता है। सहेजने से पहले अधूरा निर्देशित कट पूरा करें। बिना सहेजे बंद करने पर पुष्टि माँगी जाती है।

SONARMUSIC — विस्तृत मार्गदर्शिका

टूल्स, मल्टीमीडिया, SonarMusic खोलें या Ctrl+Alt+Shift+M दबाएँ। खोज बॉक्स में गीत, कलाकार या एल्बम का नाम लिखें। खोज प्रकार में सभी, गीत, एल्बम, कलाकार, प्लेलिस्ट या संगीत वीडियो चुनें। खोज बॉक्स में Enter या खोज बटन दबाएँ। चुना हुआ फ़िल्टर याद रखा जाता है और सुझाव उपलब्ध हो सकते हैं।

खोज परिणामों में तीरों से जाएँ और Enter दबाएँ। चलने योग्य गीत या वीडियो प्लेयर में खुलता है; एल्बम, कलाकार और प्लेलिस्ट की सामग्री खुलती है। सूची में पिछले और अधिक परिणामों के विकल्प भी रहते हैं। लोड होने पर फ़ोकस परिणामों पर लौटता है और संभव होने पर प्लेबैक के बाद पिछली खोज बहाल होती है।

चयनित परिणाम पर Applications कुंजी या Shift+F10 से संदर्भ मेनू खोलें। उपलब्ध विकल्पों में खोलना, मीडिया सहेजना/डाउनलोड करना, ट्रांसक्राइब करना, AI से ऑडियो विवरण बनाना तथा पसंदीदा में जोड़ना या हटाना शामिल हो सकता है। विकल्प मीडिया के प्रकार और सेवा की सीमाओं पर निर्भर करते हैं।

पसंदीदा सूची में सहेजे गए आइटम चुनकर Enter से खोलें; संदर्भ मेनू से हटाएँ। पीछे बटन पिछली स्क्रीन पर जाता है; Escape पीछे जाता या विंडो बंद करता है। Tab, Shift+Tab और तीरों से NVDA या JAWS के साथ काम किया जा सकता है। खोज के लिए इंटरनेट चाहिए।
''',
'_id': '''POTONG FILE MEDIA — PANDUAN LENGKAP

Buka Alat, Multimedia, Potong file media, atau tekan Alt+Shift+X. Pilih Buka file media dan tentukan berkas audio atau video lokal. Fitur ini menghapus bagian yang tidak diinginkan dan menyimpan berkas baru yang berisi bagian yang dipertahankan. Ini berbeda dari membagi satu berkas menjadi beberapa berkas.

Dalam mode Potong terpandu, pilih langkah perpindahan 1, 5, 10, 30, atau 60 detik. Ctrl+Spasi memutar atau menjeda dan Alt+Panah kiri/kanan menggeser posisi. Pada awal bagian yang akan dibuang tekan Alt+1, lalu pada akhirnya Alt+2. Alt+3 mendengarkan bagian yang dipilih, dan Alt+4 menerapkan pemotongan. Ulangi jika diperlukan.

Area perintah dirancang untuk pengguna pembaca layar. Masuk dengan Tab. Panah kiri/kanan menggeser waktu, 1 menandai awal, 2 akhir, 3 atau Enter mendengarkan, 4 memotong, Spasi memutar/menjeda, sedangkan Home/End menuju awal/akhir berkas. Shift+Panah menggeser 5 detik dan Ctrl+Panah 10 detik. F1 membacakan perintah yang tersedia; Tab keluar dari area. Pintasan Alt juga dapat digunakan di luar area.

Dalam mode lanjutan, S membagi bagian pada posisi aktif, Panah atas/bawah memilih bagian, D atau Delete menghapus, dan R memulihkan. Anda dapat menyisipkan media lain sebelum bagian terpilih dan mengatur awal/akhir dengan ketelitian 0,01 hingga 1 detik. Video dapat diputar orientasinya atau ditampilkan pratinjaunya. Trek audio tambahan juga dapat ditambahkan dengan volume masing-masing dan opsi pengulangan.

Tekan Simpan atau Ctrl+S, pilih lokasi serta format keluaran, lalu gunakan nama berbeda dari berkas asli. Kemajuan ekspor dan tombol Batal tersedia. Selesaikan potongan terpandu yang masih tertunda sebelum menyimpan. Program meminta konfirmasi jika ditutup dengan perubahan yang belum disimpan.

SONARMUSIC — PANDUAN LENGKAP

Buka Alat, Multimedia, SonarMusic, atau tekan Ctrl+Alt+Shift+M. Masukkan judul lagu, nama artis, atau album. Pilih jenis pencarian: Semua, Lagu, Album, Artis, Daftar putar, atau Video musik. Tekan Enter di kotak pencarian atau tombol Cari. Filter pilihan disimpan dan saran pencarian dapat muncul.

Jelajahi hasil dengan tombol panah dan tekan Enter. Lagu dan video yang dapat diputar terbuka dalam pemutar; album, artis, atau daftar putar membuka isinya. Daftar hasil juga berisi pilihan hasil sebelumnya atau memuat lebih banyak. Fokus kembali ke hasil setelah dimuat dan pilihan sebelumnya dapat dipulihkan seusai pemutaran.

Tekan tombol Aplikasi atau Shift+F10 pada hasil untuk membuka menu konteks. Tergantung item, tersedia Buka, Simpan/unduh media, Transkripsi, Buat deskripsi audio AI, serta Tambah atau Hapus dari favorit. Beberapa tindakan hanya tersedia untuk media yang dapat diputar dan pengunduhan mengikuti batas layanan.

Gunakan daftar Favorit untuk menemukan item tersimpan, Enter untuk membukanya, dan menu konteks untuk menghapusnya. Kembali menuju halaman sebelumnya; Escape kembali bila memungkinkan atau menutup jendela. Tab, Shift+Tab dan panah mendukung NVDA serta JAWS. Pencarian memerlukan internet.
''',
'_lt': '''MEDIJOS FAILO KIRPIMAS — IŠSAMUS VADOVAS

Atverkite Įrankiai, Multimedija, Kirpti medijos failą arba spauskite Alt+Shift+X. Pasirinkite Atverti medijos failą ir nurodykite vietinį garso ar vaizdo įrašą. Ši funkcija pašalina nereikalingas atkarpas ir išsaugo naują failą su likusia medžiaga; ji skiriasi nuo failo padalijimo į kelis atskirus failus.

Vadovaujamo kirpimo režimu pasirinkite 1, 5, 10, 30 ar 60 sekundžių žingsnį. Ctrl+Tarpas pradeda ar sustabdo atkūrimą, Alt+rodyklė kairėn/dešinėn keičia poziciją. Norimos pašalinti atkarpos pradžioje spauskite Alt+1, pabaigoje Alt+2. Alt+3 perklauso pažymėtą atkarpą, o Alt+4 ją iškerpa. Veiksmus galima kartoti.

Komandų sritis pritaikyta ekrano skaitytuvams. Pasiekite ją klavišu Tab. Rodyklės kairėn/dešinėn perkelia poziciją; 1 žymi pradžią, 2 pabaigą, 3 arba Enter perklauso, 4 iškerpa, Tarpas pradeda/stabdo, Home/End pereina į failo pradžią/pabaigą. Shift+rodyklės perkelia 5 sekundes, Ctrl+rodyklės 10 sekundžių. F1 paskelbia galimas komandas, o Tab leidžia išeiti iš srities. Alt deriniai veikia ir kitur.

Išplėstiniame režime S padalija atkarpą, rodyklės aukštyn/žemyn parenka dalį, D arba Delete ją pašalina, R atkuria. Galite įterpti kitą medijos failą prieš pasirinktą dalį, koreguoti pradžios ir pabaigos laikus 0,01–1 sekundės žingsniais, pasukti vaizdą ir įjungti peržiūrą. Taip pat galima pridėti garso takelį, atskirai reguliuoti garsumą ir kartoti papildomą takelį.

Pasirinkite Išsaugoti arba Ctrl+S, nurodykite formatą bei vietą ir įveskite kitokį pavadinimą nei originalo. Eksporto metu rodoma eiga ir atšaukimo mygtukas. Prieš išsaugodami užbaikite pradėtą kirpimą; uždarant neišsaugotus pakeitimus prašoma patvirtinimo.

SONARMUSIC — IŠSAMUS VADOVAS

Atverkite Įrankiai, Multimedija, SonarMusic arba spauskite Ctrl+Alt+Shift+M. Įrašykite dainos pavadinimą, atlikėją arba albumą. Paieškos tipai: Viskas, Dainos, Albumai, Atlikėjai, Grojaraščiai ir Muzikiniai vaizdo įrašai. Paieškos lauke spauskite Enter arba mygtuką Ieškoti. Pasirinktas filtras įsimenamas; gali būti rodomi pasiūlymai.

Rezultatus naršykite rodyklėmis ir spauskite Enter. Atkuriamos dainos ir vaizdo įrašai atveriami grotuve, o albumai, atlikėjai ir grojaraščiai atveria savo turinį. Sąraše yra perėjimo prie ankstesnių ar papildomų rezultatų įrašų. Įkėlus rezultatus fokusas grįžta į sąrašą; po atkūrimo, jei įmanoma, atkuriama ankstesnė paieška.

Paspauskite kontekstinio meniu klavišą arba Shift+F10 ant pasirinkto elemento. Pagal jo tipą siūloma Atverti, Išsaugoti/atsisiųsti, Transkribuoti, Sukurti DI garsinį aprašymą bei Pridėti prie mėgstamų ar pašalinti. Ne visos funkcijos tinka kiekvienam elementui, o atsisiuntimui taikomi paslaugos apribojimai.

Mėgstamų sąraše Enter atveria išsaugotą elementą, kontekstinis meniu leidžia jį pašalinti. Atgal grąžina į ankstesnį puslapį, Escape grįžta atgal arba uždaro langą. Tab, Shift+Tab ir rodyklės leidžia naudotis su NVDA ar JAWS. Paieškai reikalingas internetas.
''',
'_pl': '''PRZYCINANIE PLIKU MULTIMEDIALNEGO — PEŁNY PORADNIK

Otwórz Narzędzia, Multimedia, Przytnij plik multimedialny lub naciśnij Alt+Shift+X. Wybierz Otwórz plik multimedialny i wskaż lokalny plik audio albo wideo. Funkcja usuwa niechciane fragmenty i zapisuje nowy plik z zachowanymi częściami; nie jest tym samym co dzielenie pliku na kilka osobnych plików.

W trybie cięcia prowadzonego wybierz krok przewijania 1, 5, 10, 30 lub 60 sekund. Ctrl+Spacja uruchamia lub wstrzymuje odtwarzanie, Alt+strzałka w lewo/prawo przewija. Na początku usuwanego fragmentu naciśnij Alt+1, a na jego końcu Alt+2. Alt+3 odtwarza zaznaczony fragment do kontroli, Alt+4 wykonuje cięcie. Kolejne fragmenty można wycinać tak samo.

Obszar poleceń ułatwia obsługę czytnikiem ekranu. Przejdź do niego klawiszem Tab. Strzałki lewo/prawo przewijają, 1 ustawia początek, 2 koniec, 3 lub Enter odsłuchuje, 4 stosuje cięcie, Spacja odtwarza/pauzuje, Home/End przechodzą na początek/koniec pliku. Shift+strzałki przewijają o 5 sekund, Ctrl+strzałki o 10 sekund. F1 odczytuje polecenia, Tab opuszcza ten obszar. Skróty z Alt działają również poza nim.

W trybie zaawansowanym S dzieli fragment w bieżącym miejscu, strzałki góra/dół wybierają części, D lub Delete usuwa część, a R ją przywraca. Można wstawić inny plik przed wybraną częścią, poprawić czasy początku i końca krokami 0,01–1 sekundy, obrócić wideo i wyświetlić podgląd. Możliwe jest też dodanie ścieżki audio, osobne ustawienie głośności oraz zapętlenie dodanej ścieżki.

Wybierz Zapisz lub Ctrl+S, format i miejsce docelowe; podaj nazwę różną od pliku źródłowego. Podczas eksportu pokazywany jest postęp i przycisk Anuluj. Przed zapisem dokończ rozpoczęte cięcie prowadzone; przy zamykaniu niezapisanych zmian pojawia się pytanie o potwierdzenie.

SONARMUSIC — PEŁNY PORADNIK

Otwórz Narzędzia, Multimedia, SonarMusic albo Ctrl+Alt+Shift+M. Wpisz tytuł utworu, wykonawcę lub album. Filtr wyszukiwania: Wszystko, Utwory, Albumy, Wykonawcy, Playlisty albo Teledyski. Naciśnij Enter w polu wyszukiwania lub przycisk Szukaj. Filtr jest zapamiętywany, a program może podpowiadać zapytania.

Po wynikach przechodź strzałkami i naciśnij Enter. Odtwarzalne utwory i wideo otwierają się w odtwarzaczu, a albumy, wykonawcy i playlisty otwierają swoją zawartość. Lista zawiera także elementy do poprzednich i dalszych wyników. Po wczytaniu fokus wraca do wyników; po odtwarzaniu w miarę możliwości przywracane są wyszukiwanie i wybór.

Na wybranym wyniku naciśnij klawisz menu kontekstowego lub Shift+F10. Zależnie od elementu dostępne są: Otwórz, Zapisz/pobierz, Transkrybuj, Utwórz audiodeskrypcję AI oraz Dodaj/Usuń z ulubionych. Nie wszystkie działania są dostępne dla każdego rodzaju wyniku; pobieranie zależy od ograniczeń usługi.

Lista Ulubione umożliwia otwarcie zapisanego elementu Enterem albo usunięcie go z menu kontekstowego. Wstecz cofa do poprzedniego widoku, Escape cofa lub zamyka okno. Tab, Shift+Tab i strzałki wspierają NVDA oraz JAWS. Wyszukiwanie wymaga internetu.
''',
'_pt': '''CORTAR FICHEIRO MULTIMÉDIA — GUIA COMPLETO

Abra Ferramentas, Multimédia, Cortar ficheiro multimédia ou prima Alt+Shift+X. Escolha Abrir ficheiro multimédia e selecione um ficheiro local de áudio ou vídeo. Esta função elimina os excertos indesejados e guarda um novo ficheiro com o conteúdo restante. É diferente de dividir um ficheiro em vários ficheiros separados.

No Corte guiado, escolha um intervalo de deslocação de 1, 5, 10, 30 ou 60 segundos. Ctrl+Espaço inicia ou pausa a reprodução e Alt+Seta esquerda/direita desloca a posição. No início do excerto a remover prima Alt+1 e no fim prima Alt+2. Alt+3 permite ouvir o excerto selecionado e Alt+4 aplica o corte. Repita para outros excertos.

A Área de comandos foi concebida para leitores de ecrã. Aceda com Tab. Dentro dela, as setas esquerda/direita deslocam a posição; 1 marca o início, 2 o fim, 3 ou Enter permite ouvir, 4 corta, Espaço inicia/pausa e Home/End vão ao início/fim do ficheiro. Shift+Setas desloca 5 segundos e Ctrl+Setas 10 segundos. F1 anuncia os comandos disponíveis; Tab sai da área. Os atalhos Alt também funcionam fora dela.

No Corte avançado, S divide na posição atual, as setas cima/baixo selecionam partes, D ou Delete remove uma parte e R restaura-a. Pode inserir outro ficheiro multimédia antes da parte selecionada, ajustar os tempos inicial e final em incrementos de 0,01 a 1 segundo, rodar vídeo e mostrar uma pré-visualização. Também pode adicionar uma faixa de áudio, controlar o volume de cada faixa e repetir a faixa adicionada.

Prima Guardar ou Ctrl+S, escolha o formato e destino e dê ao novo ficheiro um nome diferente do original. A exportação mostra o progresso e permite cancelar. Termine o corte guiado pendente antes de guardar; ao fechar alterações não guardadas é pedida confirmação.

SONARMUSIC — GUIA COMPLETO

Abra Ferramentas, Multimédia, SonarMusic ou prima Ctrl+Alt+Shift+M. Introduza o título de uma música, artista ou álbum. Escolha o tipo de pesquisa: Tudo, Músicas, Álbuns, Artistas, Listas de reprodução ou Vídeos musicais. Prima Enter no campo de pesquisa ou o botão Pesquisar. O filtro é memorizado e podem surgir sugestões.

Percorra os resultados com as setas e prima Enter. As músicas ou vídeos reproduzíveis abrem no leitor; os álbuns, artistas e listas de reprodução abrem os respetivos conteúdos. A lista contém entradas para resultados anteriores ou mais resultados. Após o carregamento, o foco regressa aos resultados; depois da reprodução, a pesquisa e seleção são restauradas quando possível.

Na seleção, prima a tecla de Aplicações ou Shift+F10 para abrir o menu de contexto. Consoante o item, existem Abrir, Guardar/descarregar, Transcrever, Criar audiodescrição com IA e Adicionar/Remover dos favoritos. Algumas operações só existem para conteúdos reproduzíveis; ao guardar aplicam-se as opções e limitações do serviço.

Na lista Favoritos, escolha um elemento e prima Enter para o abrir ou use o menu de contexto para o remover. Voltar regressa à página anterior; Escape volta atrás quando possível ou fecha a janela. Tab, Shift+Tab e as setas permitem utilizar NVDA ou JAWS. A pesquisa necessita de Internet.
''',
'_pt_BR': '''CORTAR ARQUIVO DE MÍDIA — GUIA COMPLETO

Abra Ferramentas, Multimídia, Cortar arquivo de mídia ou pressione Alt+Shift+X. Selecione Abrir arquivo de mídia e escolha um áudio ou vídeo local. A função remove trechos indesejados e salva um novo arquivo com as partes mantidas; é diferente de dividir um arquivo em vários arquivos separados.

No Corte guiado, escolha o passo de 1, 5, 10, 30 ou 60 segundos. Ctrl+Espaço reproduz ou pausa, e Alt+Seta esquerda/direita muda a posição. No começo do trecho a remover pressione Alt+1 e no final Alt+2. Alt+3 permite ouvir o trecho marcado e Alt+4 aplica o corte. Repita para outros trechos.

A Área de comandos facilita a utilização com leitores de tela. Chegue até ela com Tab. Dentro da área, as setas esquerda/direita avançam ou retrocedem; 1 marca o início, 2 o fim, 3 ou Enter ouve, 4 aplica o corte, Espaço reproduz/pausa, e Home/End vão ao começo/fim do arquivo. Shift+Setas muda 5 segundos e Ctrl+Setas 10 segundos. F1 anuncia os comandos disponíveis; Tab sai da área. Os atalhos Alt também funcionam fora dela.

No Corte avançado, S divide na posição atual, as setas para cima/baixo selecionam partes, D ou Delete remove uma parte e R a restaura. Você pode inserir outro arquivo antes da parte selecionada, ajustar início e fim em passos de 0,01 a 1 segundo, girar o vídeo e visualizar uma prévia. Também é possível adicionar uma faixa de áudio, regular separadamente os volumes e repetir a faixa adicionada.

Pressione Salvar ou Ctrl+S, escolha o formato e o destino e dê ao novo arquivo um nome diferente do original. A exportação informa o progresso e oferece Cancelar. Finalize o corte guiado pendente antes de salvar; alterações não salvas exigem confirmação ao fechar.

SONARMUSIC — GUIA COMPLETO

Abra Ferramentas, Multimídia, SonarMusic ou pressione Ctrl+Alt+Shift+M. Digite uma música, artista ou álbum na busca. Escolha o Tipo de pesquisa: Tudo, Músicas, Álbuns, Artistas, Playlists ou Vídeos musicais. Pressione Enter no campo ou clique em Pesquisar. O filtro escolhido é lembrado e podem aparecer sugestões.

Navegue pelos resultados com as setas e pressione Enter. Músicas e vídeos reproduzíveis abrem no player; álbuns, artistas e playlists mostram seu conteúdo. A lista também possui opções para resultados anteriores ou para carregar mais. Depois do carregamento, o foco volta aos resultados e, quando possível, a pesquisa é restaurada após a reprodução.

Pressione a tecla Aplicativos ou Shift+F10 em um resultado para abrir seu menu de contexto. Dependendo do item, aparecem Abrir, Salvar/baixar mídia, Transcrever, Criar audiodescrição com IA e Adicionar ou Remover dos favoritos. Algumas ações só estão disponíveis para mídia reproduzível; downloads seguem as limitações do serviço.

Na lista Favoritos, selecione e pressione Enter para abrir ou utilize o menu de contexto para remover. Voltar retorna à página anterior; Escape volta quando possível ou fecha a janela. Tab, Shift+Tab e as setas funcionam com NVDA e JAWS. A busca requer conexão com a Internet.
''',
'_ru': '''ОБРЕЗКА МЕДИАФАЙЛА — ПОДРОБНОЕ РУКОВОДСТВО

Откройте Инструменты, Мультимедиа, Обрезка медиафайла или нажмите Alt+Shift+X. Нажмите «Открыть медиафайл» и выберите локальный аудио- или видеофайл. Функция удаляет ненужные фрагменты и сохраняет новый файл с оставшимися частями. Это не то же самое, что разделение исходного файла на несколько отдельных файлов.

В режиме пошаговой обрезки выберите шаг перемещения 1, 5, 10, 30 или 60 секунд. Ctrl+Пробел запускает и приостанавливает воспроизведение, Alt+стрелка влево/вправо перемещает позицию. В начале ненужного фрагмента нажмите Alt+1, в конце Alt+2. Alt+3 позволяет прослушать выделенное, Alt+4 применяет обрезку. Для других фрагментов повторите действия.

Область команд разработана для пользователей программ экранного доступа. Перейдите к ней клавишей Tab. Внутри стрелки влево/вправо перемещают позицию; 1 устанавливает начало, 2 конец, 3 или Enter включает прослушивание, 4 выполняет обрезку, Пробел запускает/ставит на паузу, Home/End переходят к началу/концу файла. Shift+стрелки меняют позицию на 5 секунд, Ctrl+стрелки на 10 секунд. F1 озвучивает доступные команды, Tab выходит из области. Сочетания с Alt работают и вне неё.

В расширенном режиме S разделяет часть в текущей точке, стрелки вверх/вниз выбирают части, D или Delete удаляет часть, R восстанавливает. Можно вставить другой медиафайл перед выбранной частью, настроить границы с шагом 0,01–1 секунда, повернуть видео и включить предварительный просмотр. Также можно добавить аудиодорожку, отдельно задать громкость и включить повтор добавленной дорожки.

Нажмите «Сохранить» или Ctrl+S, укажите формат и путь, выберите имя, отличное от исходного. Экспорт показывает ход выполнения и кнопку отмены. Завершите незаконченный пошаговый разрез до сохранения; при закрытии несохранённых изменений программа запрашивает подтверждение.

SONARMUSIC — ПОДРОБНОЕ РУКОВОДСТВО

Откройте Инструменты, Мультимедиа, SonarMusic или нажмите Ctrl+Alt+Shift+M. Введите название песни, исполнителя или альбома. Выберите тип поиска: Всё, Песни, Альбомы, Исполнители, Плейлисты или Музыкальные видео. Нажмите Enter в поисковой строке или кнопку Поиск. Выбранный фильтр сохраняется; возможны подсказки.

Перемещайтесь по результатам стрелками и нажмите Enter. Доступная песня или видео запускается в проигрывателе, а альбом, исполнитель или плейлист открывает содержимое. В списке также есть элементы перехода к предыдущим или следующим результатам. После загрузки фокус возвращается в список; по возможности восстанавливается прежний поиск после воспроизведения.

Для контекстного меню выбранного результата нажмите клавишу приложений или Shift+F10. В зависимости от типа доступны Открыть, Сохранить/скачать, Транскрибировать, Создать аудиоописание с ИИ, Добавить в избранное или Удалить из избранного. Некоторые действия доступны только для воспроизводимых элементов; сохранение ограничено возможностями сервиса.

В списке «Избранное» выберите элемент и нажмите Enter, чтобы открыть его, либо удалите через контекстное меню. «Назад» возвращает на предыдущую страницу; Escape возвращается или закрывает окно. Tab, Shift+Tab и стрелки обеспечивают навигацию с NVDA и JAWS. Для поиска нужен Интернет.
''',
'_sr': '''SEČENJE MEDIJSKE DATOTEKE — DETALJNO UPUTSTVO

Otvorite Alati, Multimedija, Sečenje medijske datoteke ili pritisnite Alt+Shift+X. Izaberite Otvori medijsku datoteku i lokalni audio ili video fajl. Ova funkcija uklanja neželjene delove i čuva novi fajl sa preostalim sadržajem; razlikuje se od deljenja fajla na više zasebnih fajlova.

U vođenom režimu izaberite korak pomeranja od 1, 5, 10, 30 ili 60 sekundi. Ctrl+Razmak pokreće ili pauzira reprodukciju, a Alt+strelica levo/desno menja poziciju. Na početku dela koji treba ukloniti pritisnite Alt+1, na kraju Alt+2. Alt+3 preslušava označeni deo, a Alt+4 primenjuje rez. Postupak možete ponavljati.

Komandna oblast je pogodna za čitače ekrana. Do nje dolazite tasterom Tab. Unutra strelice levo/desno pomeraju poziciju, 1 obeležava početak, 2 kraj, 3 ili Enter preslušava, 4 seče, Razmak reprodukuje/pauzira, Home/End idu na početak/kraj datoteke. Shift+strelice pomeraju 5 sekundi, Ctrl+strelice 10 sekundi. F1 izgovara dostupne komande; Tab izlazi iz oblasti. Alt prečice rade i van nje.

U naprednom režimu S deli deo na trenutnoj poziciji, strelice gore/dole biraju delove, D ili Delete uklanja deo, a R ga vraća. Možete ubaciti još jedan medijski fajl pre izabranog dela, podesiti granice u koracima od 0,01 do 1 sekunde, rotirati video i prikazati pregled. Moguće je dodati audio zapis, zasebno podesiti jačinu zvuka i ponavljati dodati zapis.

Pritisnite Sačuvaj ili Ctrl+S, odaberite format i odredište i koristite naziv različit od originala. Tokom izvoza prikazuju se napredak i dugme Otkaži. Pre čuvanja završite započeti vođeni rez; pri zatvaranju nesačuvanih izmena traži se potvrda.

SONARMUSIC — DETALJNO UPUTSTVO

Otvorite Alati, Multimedija, SonarMusic ili pritisnite Ctrl+Alt+Shift+M. Unesite pesmu, izvođača ili album u polje pretrage. Izaberite tip pretrage: Sve, Pesme, Albumi, Izvođači, Plejliste ili Muzički video snimci. Pritisnite Enter u polju ili dugme Pretraži. Izabrani filter se pamti i mogu se ponuditi predlozi.

Kroz rezultate se krećite strelicama i pritisnite Enter. Pesme i video snimci koji se mogu pustiti otvaraju se u plejeru, a albumi, izvođači i plejliste otvaraju svoj sadržaj. Lista sadrži i stavke za prethodne i dodatne rezultate. Posle učitavanja fokus se vraća na rezultate; po završetku reprodukcije pretraga se po mogućstvu obnavlja.

Na stavci pritisnite taster Aplikacije ili Shift+F10 za kontekstni meni. U zavisnosti od vrste dostupni su Otvori, Sačuvaj/preuzmi medij, Transkribuj, Napravi audio-opis uz AI i Dodaj ili Ukloni iz omiljenih. Neke radnje rade samo sa sadržajem za reprodukciju i podležu ograničenjima usluge.

U listi Omiljeno odaberite stavku i pritisnite Enter za otvaranje ili je uklonite kroz kontekstni meni. Nazad vraća na prethodnu stranicu; Escape se vraća ili zatvara prozor. Tab, Shift+Tab i strelice omogućavaju rad sa NVDA i JAWS. Za pretragu je potreban internet.
''',
'_sv': '''KLIPP MEDIEFIL — FULLSTÄNDIG GUIDE

Öppna Verktyg, Multimedia, Klipp mediefil, eller tryck Alt+Skift+X. Välj Öppna mediefil och en lokal ljud- eller videofil. Funktionen tar bort oönskade avsnitt och sparar en ny fil med det material som behålls. Det är inte samma sak som att dela en fil i flera separata filer.

I guidat klippläge väljer du ett förflyttningssteg på 1, 5, 10, 30 eller 60 sekunder. Ctrl+Mellanslag startar/pausar uppspelningen; Alt+vänster-/högerpil flyttar positionen. Tryck Alt+1 i början av avsnittet som ska bort och Alt+2 i slutet. Alt+3 spelar upp det markerade avsnittet för kontroll och Alt+4 tillämpar klippet. Upprepa vid behov.

Kommandofältet är anpassat för skärmläsare. Gå dit med Tab. Där flyttar vänster-/högerpilarna positionen, 1 markerar början, 2 slutet, 3 eller Retur lyssnar, 4 klipper, Mellanslag startar/pausar och Home/End går till filens början/slut. Skift+pilar flyttar 5 sekunder och Ctrl+pilar 10 sekunder. F1 läser upp tillgängliga kommandon; Tab lämnar fältet. Alt-genvägarna fungerar även utanför det.

I avancerat läge delar S vid aktuell position, upp-/nedpilar väljer delar, D eller Delete tar bort en del och R återställer den. Du kan infoga en annan mediefil före den valda delen, justera start och slut i steg på 0,01–1 sekund, rotera video och visa förhandsgranskning. Du kan också lägga till ett ljudspår, justera volymer separat och upprepa det tillagda spåret.

Tryck Spara eller Ctrl+S, välj format och plats och ge den nya filen ett annat namn än originalet. Exporten visar förlopp och har en Avbryt-knapp. Slutför ett påbörjat guidat klipp innan du sparar; vid stängning av osparade ändringar krävs bekräftelse.

SONARMUSIC — FULLSTÄNDIG GUIDE

Öppna Verktyg, Multimedia, SonarMusic eller tryck Ctrl+Alt+Skift+M. Skriv en låt, artist eller ett album i sökrutan. Välj söktyp: Alla, Låtar, Album, Artister, Spellistor eller Musikvideor. Tryck Retur i sökrutan eller knappen Sök. Vald filtrering sparas och sökförslag kan visas.

Bläddra med piltangenterna bland sökresultaten och tryck Retur. Spelbara låtar och videor öppnas i spelaren; album, artister och spellistor visar sitt innehåll. Listan har även poster för föregående eller fler resultat. Efter inläsning återgår fokus till resultaten och efter uppspelning återställs om möjligt tidigare sökning.

Tryck Programtangenten eller Skift+F10 på ett valt resultat för snabbmenyn. Beroende på innehåll finns Öppna, Spara/ladda ned media, Transkribera, Skapa AI-ljudbeskrivning samt Lägg till/Ta bort favorit. Vissa åtgärder finns bara för spelbara objekt och nedladdning påverkas av tjänstens begränsningar.

I Favoriter väljer du ett sparat objekt och trycker Retur för att öppna eller tar bort det via snabbmenyn. Tillbaka återgår till föregående sida; Escape går tillbaka eller stänger fönstret. Tab, Skift+Tab och pilar stöder NVDA och JAWS. Sökningen kräver internet.
''',
'_uk': '''ОБРІЗАННЯ МЕДІАФАЙЛУ — ПОВНИЙ ПОСІБНИК

Відкрийте Інструменти, Мультимедіа, Обрізати медіафайл або натисніть Alt+Shift+X. Оберіть Відкрити медіафайл і локальний аудіо- чи відеофайл. Ця функція видаляє непотрібні фрагменти й зберігає новий файл з рештою матеріалу. Це не те саме, що поділ файлу на декілька окремих файлів.

У режимі покрокового обрізання виберіть крок 1, 5, 10, 30 або 60 секунд. Ctrl+Пробіл запускає чи призупиняє відтворення, Alt+стрілка ліворуч/праворуч пересуває позицію. На початку непотрібного фрагмента натисніть Alt+1, у кінці Alt+2. Alt+3 дає прослухати виділений фрагмент, Alt+4 застосовує обрізання. Для інших фрагментів повторіть дії.

Область команд створена для користувачів програм екранного доступу. Перейдіть до неї клавішею Tab. Тут стрілки ліворуч/праворуч пересувають позицію, 1 позначає початок, 2 кінець, 3 або Enter слухає, 4 обрізає, Пробіл запускає/призупиняє, Home/End переходять на початок/кінець файлу. Shift+стрілки переміщують на 5 секунд, Ctrl+стрілки на 10 секунд. F1 озвучує доступні команди, Tab виходить з області. Сполучення з Alt працюють і поза нею.

У розширеному режимі S ділить частину в поточній позиції, стрілки вгору/вниз вибирають частини, D або Delete видаляє частину, R відновлює. Можна вставити інший медіафайл перед вибраною частиною, налаштувати початок і кінець з кроком 0,01–1 секунди, повернути відео та показати попередній перегляд. Можна також додати аудіодоріжку, окремо налаштувати гучність і повтор доданої доріжки.

Натисніть Зберегти або Ctrl+S, виберіть формат і місце та ім'я, відмінне від оригіналу. Під час експорту показується поступ і кнопка скасування. Перед збереженням завершіть розпочате покрокове обрізання; закриття з незбереженими змінами потребує підтвердження.

SONARMUSIC — ПОВНИЙ ПОСІБНИК

Відкрийте Інструменти, Мультимедіа, SonarMusic або натисніть Ctrl+Alt+Shift+M. Введіть назву пісні, виконавця чи альбому. Тип пошуку: Усе, Пісні, Альбоми, Виконавці, Списки відтворення або Музичні відео. Натисніть Enter у полі чи кнопку Пошук. Обраний фільтр запам'ятовується; можуть з'являтися підказки.

Переглядайте результати стрілками і натискайте Enter. Доступні для відтворення пісні та відео відкриваються в програвачі, альбоми, виконавці й списки відтворення показують вміст. У списку є пункти для попередніх або додаткових результатів. Після завантаження фокус повертається до результатів, після відтворення за можливості відновлюється попередній пошук.

На вибраному елементі натисніть клавішу контекстного меню або Shift+F10. Залежно від типу доступні Відкрити, Зберегти/завантажити, Транскрибувати, Створити аудіоопис за допомогою ШІ, Додати до обраного або Видалити з обраного. Не всі дії доступні для кожного елемента; збереження залежить від обмежень сервісу.

У списку Обране виберіть елемент і натисніть Enter або видаліть його через контекстне меню. Назад повертає на попередню сторінку; Escape повертає назад або закриває вікно. Tab, Shift+Tab і стрілки підтримують NVDA та JAWS. Для пошуку потрібен інтернет.
''',
'_vi': '''CẮT TỆP ĐA PHƯƠNG TIỆN — HƯỚNG DẪN ĐẦY ĐỦ

Mở Công cụ, Đa phương tiện, Cắt tệp đa phương tiện hoặc nhấn Alt+Shift+X. Chọn Mở tệp đa phương tiện rồi chọn tệp âm thanh hay video trên máy. Tính năng này loại bỏ các đoạn không mong muốn và lưu tệp mới chỉ chứa phần được giữ lại; không giống việc chia tệp thành nhiều tệp riêng biệt.

Ở chế độ Cắt có hướng dẫn, chọn bước di chuyển 1, 5, 10, 30 hoặc 60 giây. Ctrl+Phím cách phát/tạm dừng; Alt+Mũi tên trái/phải di chuyển vị trí. Đến đầu đoạn cần xóa, nhấn Alt+1; đến cuối đoạn, nhấn Alt+2. Alt+3 nghe thử đoạn đã chọn và Alt+4 áp dụng cắt. Có thể lặp lại cho các đoạn khác.

Vùng lệnh được thiết kế để dùng với trình đọc màn hình. Nhấn Tab để đến vùng này. Tại đây, mũi tên trái/phải tua, phím 1 đánh dấu đầu, 2 đánh dấu cuối, 3 hoặc Enter nghe thử, 4 cắt, Phím cách phát/tạm dừng, Home/End đến đầu/cuối tệp. Shift+Mũi tên tua 5 giây, Ctrl+Mũi tên tua 10 giây. F1 đọc các lệnh khả dụng; Tab rời vùng lệnh. Phím tắt Alt cũng hoạt động bên ngoài vùng này.

Ở chế độ nâng cao, S chia đoạn tại vị trí hiện tại, mũi tên lên/xuống chọn đoạn, D hoặc Delete xóa đoạn và R khôi phục. Có thể chèn tệp khác trước đoạn được chọn, chỉnh điểm đầu/cuối theo bước 0,01 đến 1 giây, xoay video và bật xem trước. Ngoài ra có thể thêm bản âm thanh, điều chỉnh âm lượng riêng và lặp bản mới khi cần.

Nhấn Lưu hoặc Ctrl+S, chọn định dạng và nơi lưu, đặt tên khác tệp gốc. Khi xuất tệp có tiến trình và nút Hủy. Hãy hoàn tất thao tác cắt có hướng dẫn còn dang dở trước khi lưu; đóng khi chưa lưu sẽ hỏi xác nhận.

SONARMUSIC — HƯỚNG DẪN ĐẦY ĐỦ

Mở Công cụ, Đa phương tiện, SonarMusic hoặc nhấn Ctrl+Alt+Shift+M. Nhập tên bài hát, nghệ sĩ hoặc album vào ô tìm kiếm. Chọn loại tìm kiếm: Tất cả, Bài hát, Album, Nghệ sĩ, Danh sách phát hoặc Video âm nhạc. Nhấn Enter trong ô hoặc nút Tìm kiếm. Bộ lọc được ghi nhớ và có thể có gợi ý.

Dùng phím mũi tên để duyệt kết quả và nhấn Enter. Bài hát và video phát được mở trong trình phát; album, nghệ sĩ và danh sách phát mở nội dung bên trong. Danh sách cũng có mục kết quả trước và tải thêm. Sau khi tải, tiêu điểm quay lại kết quả; sau phát, chương trình cố gắng khôi phục tìm kiếm và lựa chọn trước đó.

Nhấn phím Ứng dụng hoặc Shift+F10 trên kết quả đã chọn để mở menu ngữ cảnh. Tùy loại mục có thể có Mở, Lưu/tải phương tiện, Phiên âm, Tạo mô tả âm thanh bằng AI và Thêm/Xóa khỏi yêu thích. Một số tác vụ chỉ khả dụng cho nội dung phát được; việc tải xuống chịu giới hạn của dịch vụ.

Trong danh sách Yêu thích, chọn mục rồi nhấn Enter để mở hoặc dùng menu ngữ cảnh để xóa. Quay lại trở về trang trước; Escape quay lại khi có thể hoặc đóng cửa sổ. Tab, Shift+Tab và phím mũi tên hỗ trợ NVDA và JAWS. Tìm kiếm cần Internet.
''',
'_zh': '''剪辑媒体文件 — 完整指南

打开“工具”→“多媒体”→“剪辑媒体文件”，或按 Alt+Shift+X。选择“打开媒体文件”，加载本地音频或视频。此功能用于删除不需要的片段，并把保留的内容保存为新文件；它与将文件分割成多个独立文件不同。

在“引导剪辑”模式下，选择每次移动 1、5、10、30 或 60 秒。Ctrl+空格键用于播放或暂停，Alt+左/右方向键用于定位。到达要删除片段的起点时按 Alt+1，到达终点时按 Alt+2。按 Alt+3 试听选中的待删除片段，再按 Alt+4 应用剪辑。如需删除更多片段，可重复以上步骤。

“命令区域”专门方便屏幕阅读器用户操作。用 Tab 移入该区域；左/右方向键移动播放位置，数字 1 标记起点，2 标记终点，3 或回车键试听，4 执行剪辑，空格键播放/暂停，Home/End 跳至文件开始/结束。Shift+左右方向键每次移动 5 秒，Ctrl+左右方向键每次移动 10 秒。按 F1 朗读当前模式可用的命令，用 Tab 离开命令区域。带 Alt 的快捷键也可在区域外使用。

在“高级剪辑”模式中，按 S 在当前位置拆分片段，用上/下方向键选择片段，按 D 或 Delete 删除，按 R 恢复。可以在选中片段前插入另一媒体文件，以 0.01、0.1、0.5 或 1 秒为步长精调起止点，还可旋转视频或显示预览。也可以添加音轨，分别调整原音轨和新音轨的音量，并设置新音轨循环播放。

按“保存”或 Ctrl+S，选择输出格式与位置，并使用不同于原文件的名称。导出时会显示进度及取消按钮。保存前应完成尚未应用的引导剪辑；关闭未保存的编辑内容时会询问确认。

SONARMUSIC — 完整指南

打开“工具”→“多媒体”→“SonarMusic”，或按 Ctrl+Alt+Shift+M。在搜索框中输入歌曲、歌手或专辑名称。搜索类型可选：全部、歌曲、专辑、歌手、播放列表或音乐视频。在搜索框按回车键或选择“搜索”。程序会记住筛选类型，并可能提供搜索建议。

使用方向键浏览搜索结果，按回车键打开。可播放的歌曲或视频会在播放器中打开；专辑、歌手或播放列表则显示各自的内容。结果列表还包含上一页和加载更多的选项。加载后焦点会返回结果；播放结束后会尽可能恢复此前的搜索和选择位置。

在选中的结果上按应用程序键或 Shift+F10 打开右键菜单。根据项目类型，可以选择打开、保存/下载媒体、转录、使用 AI 创建音频描述，以及添加到收藏或从收藏中移除。有些操作只适用于可播放的内容；保存媒体时须遵循服务提供的选项及限制。

在“收藏”列表中选择项目并按回车键打开，使用右键菜单可将其移除。“返回”可回到上一页；Escape 会在可能时返回，否则关闭窗口。Tab、Shift+Tab 和方向键可配合 NVDA 或 JAWS 使用。音乐搜索需要互联网连接。
'''
}


def main():
    expected = {f'guida{suffix}.txt' for suffix in SECTIONS}
    actual = {file.name for file in ROOT.glob('guida*.txt')}
    if actual != expected:
        raise RuntimeError(f'Guide mancanti/aggiunte: {sorted(actual ^ expected)}')
    planned = []
    for suffix, section in SECTIONS.items():
        path = ROOT / f'guida{suffix}.txt'
        raw = path.read_bytes()
        eol = b'\r\n' if b'\r\n' in raw else b'\n'
        has_bom = raw.startswith(b'\xef\xbb\xbf')
        text = raw.decode('utf-8-sig').replace('\r\r\n', '\n').replace('\r\n', '\n')
        if MARKER in text:
            if b'\r\r\n' in raw:
                repaired = raw.replace(b'\r\r\n', b'\r\n')
                planned.append((path, repaired))
                print(f'CORREZIONE TERMINAZIONI RIGA: {path.name}')
            else:
                print(f'GIA PRESENTE: {path.name}')
            continue
        if not ('SONARMUSIC' in section and section.count('\n\n') >= 8 and 'Alt+' in section and ('Ctrl+' in section or 'Strg+' in section)):
            # Le combinazioni sono tradotte con i nomi locali dei modificatori.
            raise RuntimeError(f'Sezione incompleta: {path.name}')
        addition = MARKER + '\n\n' + section.strip() + '\n'
        if suffix == '_de':
            end = 'ENDE DES HANDBUCHS'
        elif suffix == '_pt_BR':
            end = 'FIM DO GUIA'
        else:
            end = None
        if end:
            if text.count(end) != 1:
                raise RuntimeError(f'Segnaposto finale non univoco: {path.name}')
            revised = text.replace(end, addition + '\n' + end)
        else:
            revised = text + '\n' + addition
        if has_bom:
            revised = revised.encode('utf-8-sig')
        else:
            revised = revised.encode('utf-8')
        revised = revised.replace(b'\n', eol)
        planned.append((path, revised))
    for path, content in planned:
        path.write_bytes(content)
        print(f'AGGIORNATA: {path.name} ({len(content)} byte)')
    print(f'Totale guide aggiornate: {len(planned)}; guide verificate: {len(SECTIONS)}')


if __name__ == '__main__':
    main()

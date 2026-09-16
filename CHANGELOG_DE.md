# Änderungsprotokoll

Version 0.9.10 – 2026-09-16

Audiodeskriptionen
1. Audiodeskriptionen wurden verbessert, indem das Zeichen `_` (Unterstrich) durch ein Leerzeichen ersetzt wird, damit die Sprachsynthese dieses Zeichen in Audiodeskriptionen nicht ausspricht.

Version 0.9.9 – 2026-09-15

KI-Audiodeskription
1. Ein isolierter Fallback für Quellvideos ohne Audiospur wurde hinzugefügt, ohne die bereits funktionierende Pipeline zu verändern. Sonarpad versucht weiterhin zuerst den normalen Export. Nur wenn FFmpeg einen fehlenden Audiostream meldet und Sonarpad bestätigt, dass die Quelle tatsächlich keine Audiospur enthält, wird eine stille Quelle mit gleicher Dauer erzeugt und die synthetisierten Beschreibungen werden darauf gemischt. Beim Export auf das Originalvideo wird anschließend der bestehende Video+Audio-Mux-Pfad wiederverwendet. Videos mit vorhandener Audiospur nutzen unverändert den normalen Pfad.

2. Den strikt isolierten Fallback für den Fall ohne nutzbare Beschreibungen erweitert, ohne die normale Audiodeskriptions-Pipeline zu verändern. Sonarpad führt weiterhin zuerst die bestehende Analyse aus; nur wenn danach keine nutzbare Beschreibung vorhanden ist, wird ein zweiter Versuch mit der Einstellung Kurz angeboten, der dieselbe Pipeline verwendet und alle Pausen- sowie Nicht-Überlappungsregeln beibehält. Kann auch dieser zweite Versuch keine Beschreibung platzieren, fragt Sonarpad jetzt ausdrücklich nach der Zustimmung zu einem letzten Fallback. Erst nach Ja werden die bereits von Gemini erzeugten kurzen Beschreibungen aus dem Checkpoint des zweiten Versuchs wiederverwendet und mit Ducking an ihren visuellen Zeitpunkten gemischt, wobei kurze Überlappungen mit Dialog erlaubt sind. Pyannote, Gemini-Bridge, normale Ausrichtungsregeln, TTS-Planung und die ersten beiden Analysepfade bleiben unverändert. Bei Ablehnung oder fehlenden wiederverwendbaren Beschreibungen beendet Sonarpad den Vorgang sauber mit einer lokalisierten Meldung.

3. Der isolierte Fallback für fehlende nutzbare Beschreibungen wurde verfeinert, ohne die normale Audiodeskriptions-Pipeline zu verändern. Wenn die ursprünglich gewählte Ausführlichkeit bereits Kurz ist und die Analyse ohne nutzbare Beschreibung endet, überspringt Sonarpad jetzt die redundante zweite Gemini-Analyse und fragt bei vorhandenen wiederverwendbaren Beschreibungen direkt nach dem finalen Fallback mit Dialogüberlappung. War die ursprüngliche Ausführlichkeit Standard oder Detailliert, bleibt der Kurz-Versuch auch bei sehr kurzen Pausen erhalten: Er dient dann zusätzlich dazu, eine kürzere Beschreibung für den möglichen finalen Fallback zu erzeugen, damit die Erzählung den Dialog möglichst kurz überlagert. Pyannote, die Gemini-Bridge und die normalen Regeln für Pausen und Ausrichtung bleiben unverändert.

Podcast-Aufnahme
1. Ein konservativer Fallback für fehlerhafte Gerätepositionsmeldungen bestimmter WASAPI-Mikrofontreiber wurde hinzugefügt, ohne funktionierende Aufnahmen zu verändern. Er aktiviert sich nur bei dauerhaft falschen Positionswerten: 24 aufeinanderfolgenden Abweichungen oder mindestens 80 % Abweichungen in 64 sauberen Paketen. Echte WASAPI-`DATA_DISCONTINUITY`-Meldungen und Zeitstempelfehler bleiben maßgeblich; die Systemaudio-Aufnahme bleibt unverändert.

YouTube und Streaming
1. Fehlgeschlagene YouTube-Downloads aus dem Kontextmenü der Ergebnisse wurden korrigiert. Bekannte yt-dlp-Fehler für Inhalte nur für Kanalmitglieder werden nun in die lokalisierte Sonarpad-Meldung umgewandelt, statt den rohen englischen Text anzuzeigen. Nach dem Schließen der Meldung stellt Sonarpad den Fokus erst nach dem Ende der nativen Kontextmenü-/Modal-Schleife wieder her und kehrt zuverlässig mit aktiver Tastatur zur Ergebnisliste zurück. Die normale erfolgreiche Download-Pipeline bleibt unverändert.

2. Der vorbeugende Filter wurde an SonarTube Mobile angeglichen: In Suchergebnissen sowie beim Durchsuchen von Kanälen und Playlists werden Videos ausgeblendet, für die YouTube/yt-dlp keine Aufrufzahl liefert. Kanäle und Playlists bleiben sichtbar, und echte Videos mit 0 Aufrufen werden nicht entfernt. Die lokalisierte Fehlerbehandlung für Inhalte nur für Mitglieder bleibt als zweiter Fallback aktiv. Der Mehrfachdownload von Playlists bleibt unverändert.

Version 0.9.8 – 2026-09-12

KI-Audiodeskription
1. Eine konservative Wiederherstellung für den Mehrkanal-Export wurde hinzugefügt, ohne die Pipeline für bereits funktionierende Dateien zu ändern. Sonarpad verwendet weiterhin zuerst das ursprüngliche Kanallayout und ergänzt nur bei Bedarf einen unvollständigen letzten PCM-Frame. Falls ein Mehrkanal-Zwischen-WAV dennoch scheitert, weil die Anzahl der Samples kein Vielfaches der Kanalzahl ist, wird nur dieser Export automatisch in Stereo wiederholt. Erfolgreiche Mono-, Stereo- und Mehrkanal-Exporte bleiben unverändert.

Dunkles Design und Barrierefreiheit
1. Behoben, dass das dunkle Design native Windows-Dialoge und Bestätigungsmeldungen beeinträchtigte. Sonarpad wendet sein benutzerdefiniertes Dark-Theme-Subclassing nicht mehr auf Systemdialogbäume der Klasse `#32770` an. Öffnen-/Speichern-Dialoge, der Diagnoseexport und MessageBox-Bestätigungen bleiben damit unter Windows-Verwaltung und erhalten wieder korrekt Tastatur-/NVDA-Fokus. Außerdem wurde die Lebensdauer der Standard-ZIP-Erweiterung im Speichern-Dialog der Diagnose korrigiert.

Version 0.9.7 – 2026-09-11

KI-Audiodeskription
1. Die optionale Einstellung „Audiodeskription im ursprünglichen Video speichern“ wurde hinzugefügt. Ist sie aktiviert, bevorzugt Sonarpad jetzt MP4: Der ursprüngliche Videostream wird ohne Neucodierung kopiert und die gemischte Audiodeskriptionsspur eingefügt. Meldet FFmpeg beim Erstellen der MP4-Datei einen Kompatibilitätsfehler des Containers oder beim Schreiben der Pakete, wiederholt Sonarpad denselben Export automatisch als MKV, weiterhin ohne das Video neu zu codieren. MKV kann außerdem ausdrücklich gewählt werden. Erweiterte Pausen werden in diesem Modus ignoriert, damit Audio und Video synchron bleiben.

2. Die Segment-Neuanalyse in gespeicherten Audiodeskriptionsprojekten wurde verbessert. Das Fenster verwendet jetzt den KI-Zugang, der bereits im Hauptfenster „Audiodeskription erstellen“ konfiguriert ist; API-Schlüssel, Sonarpad-AI-Guthaben und Modellauswahl werden daher nicht mehr doppelt angezeigt. Ein neu analysiertes Segment behält nun die gesamte Gruppe der Projektbeschreibungen, die zu diesem Analyse-Chunk gehören, statt nur die erste bzw. nächstgelegene Beschreibung. Alle Beschreibungen der Gruppe werden als neu analysiert markiert und „Segment anwenden und MP3 erneut exportieren“ übernimmt die gesamte Gruppe in einem Schritt. Vorschauen erweiterter Pausen werden direkt synthetisiert, statt in der exportierten MP3-Datei zu suchen, wodurch Vorschauen vermieden werden, die mit Filmaudio beginnen oder die Beschreibung abschneiden.

3. Der konservative Gemini-Medien-Fallback wurde erweitert, ohne den bereits funktionierenden Ablauf zu ändern. HTTP 400 `INVALID_ARGUMENT` verwendet weiterhin kleinere MKV-Chunks (etwa 15 MB) und anschließend kompatible MP4-Chunks. Wenn Gemini einen hochgeladenen Chunk akzeptiert, die Verarbeitung aber mit `FAILED` endet, lädt Sonarpad genau diesen Chunk einmal erneut hoch und wechselt erst danach zum MP4-Fallback; Server-Code-13-Wiederholungen sind auf drei begrenzt, bevor derselbe Fallback verwendet wird, damit keine Endlosschleife entsteht. Netzwerk-, Kontingent-, Authentifizierungs- und Synthesefehler aktivieren diesen Weg nicht.

4. Die Speicherung der KI-Zugangsdaten beim Wechsel des Zugriffsmodus wurde korrigiert. Der persönliche Gemini-API-Schlüssel und der Sonarpad-AI-Zugangscode werden jetzt unabhängig voneinander gespeichert: Beim Wechsel zwischen persönlichem API-Schlüssel und Sonarpad AI wird die jeweils inaktive Zugangsdaten nicht mehr gelöscht. Der Gemini-Schlüssel wird außerdem gespeichert, sobald das Feld den Fokus verliert.

5. Für „Segment erneut analysieren“ und „Segment anwenden und MP3 erneut exportieren“ wurde eine echte Abbrechen-Schaltfläche hinzugefügt. Die Fortschrittsanzeige bleibt aktiv; Abbrechen beendet nun die FFmpeg-Segmentvorbereitung, den KI-Worker, die TTS-Prüfung und den abschließenden Export, sobald die jeweilige Phase sicher gestoppt werden kann. Das Anwenden eines erneut analysierten Segments erfolgt jetzt transaktional: Das vorhandene Projekt und die Ausgabedatei werden erst nach erfolgreichem Neu-Export ersetzt. Bei Abbruch bleiben die bisherigen Dateien unverändert und der Vorschlag zur erneuten Analyse bleibt für einen weiteren Versuch verfügbar.

6. Die Segment-Neuanalyse für Beschreibungen außerhalb des ersten Gemini-Chunks wurde korrigiert. Isolierte Chunks werden nun mit einer lokalen, bei 0 beginnenden Zeitleiste an den bestehenden Worker gesendet und anschließend wieder auf ihre absolute Projektposition abgebildet. Dadurch wird `Invalid prepared chunk timeline at chunk 1` vermieden, ohne die normale Audiodeskriptions-Pipeline zu ändern.

7. Die Segment-Neuanalyse gespeicherter Projekte wurde so überarbeitet, dass sie dieselben Sicherheitsprüfungen wie die vollständige Audiodeskriptions-Erstellung verwendet. Der ausgewählte Abschnitt durchläuft nun dieselbe Mono-16-kHz-Audioextraktion und Pyannote-Dialog-/Pausenanalyse, dieselben Gemini-Zeitregeln und Medien-Fallbacks, dieselbe reale TTS-Synthese mit der Projektstimme sowie denselben abschließenden Scheduler für sichere Platzierung und erweiterte Pausen. Beim Anwenden werden die neu geprüften absoluten Zeiten und die aktualisierten Pyannote-Schutzintervalle des Abschnitts übernommen; die vorherige spezielle Vorschau-Umgehung für zu lange neu analysierte Texte wurde entfernt.

8. Die strukturelle Ersetzung eines Segments nach der vollständigen Neuanalyse wurde korrigiert. Sonarpad verlangt nicht mehr, dass das neu geprüfte Segment genau dieselbe Anzahl an Beschreibungen wie das gespeicherte Projekt enthält: Die alten Beschreibungen dieses Chunks werden durch das vollständige sichere Ergebnis der Pipeline ersetzt, sodass aus 10 Beschreibungen korrekt 9 (oder 11) werden können. Der Projekteditor zeigt die neue Anzahl und die neuen Zeiten sofort zur Vorschau; gespeichertes Projekt und Ausgabedatei bleiben bis zum erfolgreichen Abschluss von „Segment anwenden und MP3 erneut exportieren“ unverändert.

9. Die Segment-Neuanalyse wurde so überarbeitet, dass der ausgewählte physische Abschnitt wie ein echter, eigenständiger Minifilm behandelt wird. Sonarpad übergibt diesen Minifilm jetzt direkt an exakt dieselbe Funktion `create_audio_description`, die auch bei der normalen vollständigen Erstellung verwendet wird. Dadurch teilen Video und Tonspur dieselbe lokale Zeitleiste und die normalen Pyannote-, Gemini-, Echt-TTS-, Planungs- und Sicherheitsprüfungen laufen ohne separate Neuanalyse-Implementierung. Erst nach Abschluss dieser normalen Pipeline werden die lokalen Zeiten wieder auf die ursprüngliche Position im Film verschoben.

Textbearbeitung
1. „Umgebrochene Zeilen verbinden“ unter Bearbeiten > Text wurde verbessert. Ohne Auswahl wird das gesamte Dokument verarbeitet, mit Auswahl nur die markierten Zeilen. Aufeinanderfolgende Zeilen werden nun auch nach Satzzeichen zu vollständigen Absätzen zusammengeführt; Leerzeilen bleiben Absatztrenner und nummerierte sowie Aufzählungslisten werden beibehalten.

10. Die zeitliche Zuordnung bei der Segment-Neuanalyse wurde korrigiert, wenn der analysierte Mini-Film wieder auf den Originalfilm abgebildet wird. Sonarpad verwendet nun dieselbe kleine Chunk-Dauer-Abstimmung wie bei der vollständigen Analyse auch für Beschreibungen, Visual-Evidence-Zeitpunkte und geschützte Dialogintervalle, damit sich gegen Ende eines Chunks keine Verschiebung in den Dialog einschleicht.

Version 0.9.6 – 2026-09-09

1. Blockierungen einiger SAPI4-Stimmen bei aktivierter Cursorverfolgung behoben. Die Bridge wartet jetzt auf den tatsächlichen Abschluss der Synthese; die Cursorverfolgung bleibt aktiv.

2. Synchronisierung von Mikrofon und Systemaudio sowie durch Zeitstempelrundung verursachtes Knacken bei Podcast-Aufnahmen korrigiert. Die Korrekturen gelten auch beim getrennten Speichern der Quellen.

3. Die SAPI5-Synthese für Audiodeskriptionen läuft jetzt in separaten Prozessen. Fehlgeschlagene Synthesen werden mit weniger parallelen Prozessen wiederholt; erfolgreiche Beschreibungen bleiben erhalten und die funktionierende Grenze wird für die Stimme gespeichert.

Version 0.9.5 – 2026-09-07

KI-Audiodeskription
1. Die Robustheit wurde verbessert, wenn Gemini vorübergehend `MALFORMED_RESPONSE` ohne verwertbaren Inhalt zurückgibt. Sonarpad versucht nun exakt denselben Chunk bis zu dreimal erneut und wartet kurz zwischen den Versuchen, anstatt die gesamte Audiodeskription sofort abzubrechen. Normale Gemini-Antworten, Sicherheitsblockierungen, Tokenlimit-Fehler und das bestehende Checkpoint-Verhalten bleiben unverändert.

2. Ein weiterer seltener Fall von `invalid Gemini chunk timeline` wurde behoben, der bei einigen Videos auftreten konnte, wenn von FFmpeg vorbereitete Chunks eine kleine kumulierte Abweichung der Dauer meldeten. Sonarpad lässt den bisherigen Ablauf unverändert, solange die Timeline gültig ist, und verwendet die vorsichtige Korrektur nur dann, wenn die normale Timeline fehlschlagen würde und die Gesamtabweichung klein ist; große oder verdächtige Abweichungen führen weiterhin zu einem Fehler.

3. Der optionale Sonarpad-AI-Dienst für KI-Audiodeskriptionen wurde hinzugefügt. Nutzer können weiterhin ihren eigenen Gemini-API-Schlüssel verwenden oder den Sonarpad-Dienst auswählen und einen persönlichen Sonarpad-Code eingeben. Der Code wird mit Windows DPAPI geschützt. Im Dienstmodus werden vorbereitete Video-Chunks über eine temporäre Google-Upload-URL direkt vom PC des Nutzers zu Google hochgeladen; sonarpad.com empfängt oder speichert keine Videodaten. Das Backend autorisiert nur den Zugriff, erzwingt Gemini-Modell/Standard-Tier und Nutzungslimits, erfasst den Verbrauch und gibt die Gemini-Antwort zurück.

4. Die Bearbeitung gespeicherter Audiodeskriptionsprojekte wurde verbessert. Mehrere Beschreibungen können nacheinander geändert werden, ohne jede Änderung sofort anwenden zu müssen. Die Änderungen bleiben im Speicher; mit „Text anwenden“ prüft Sonarpad jede geänderte Beschreibung gegen ihren gespeicherten Zeitbereich und speichert danach alle gültigen Änderungen gemeinsam. Passt eine Beschreibung nicht in ihren Bereich, kehrt der Fokus zu dieser Beschreibung zurück, ohne die anderen noch nicht angewendeten Änderungen zu verlieren.

5. „Stimme anpassen“ wurde direkt vor „Audiodeskription erstellen“ hinzugefügt. Das neue Fenster bietet Auswahl für Sprachsynthese-Engine, Stimme, Geschwindigkeit und Lautstärke sowie „Stimme testen“. Nach OK kehrt der Fokus genau zu „Stimme anpassen“ zurück und die gewählten Werte werden für die zu erstellende Audiodeskription verwendet. Wird das Fenster nicht benutzt, bleibt das bisherige Syntheseverhalten unverändert.


6. Die Sprachsteuerung in gespeicherten Audiodeskriptionsprojekten wurde erweitert. Unter Projekt bearbeiten stehen neben Engine und Stimme jetzt auch Geschwindigkeit, Lautstärke und „Stimme testen“ zur Verfügung. Beim Anwenden der Spracheinstellungen prüft Sonarpad alle vorhandenen Beschreibungen mit den neuen Syntheseparametern erneut und erstellt die MP3 nur neu, wenn weiterhin alle Beschreibungen in ihre gespeicherten Zeitbereiche passen. Dialogfreie Intervalle, Visual-Evidence-Zeitpunkte, Gemini-Beschreibungen und die Zeitanalyse des Projekts werden wiederverwendet, ohne die KI-Analyse erneut auszuführen.

Version 0.9.4 – 2026-09-04

KI-Audiodeskription
1. Ein Problem mit Matroska-/WebM-Videos wurde behoben, die einen ungewöhnlich hohen internen Startzeitstempel enthalten und die Erstellung der Audiodeskription mit dem Fehler „invalid Gemini chunk timeline“ abbrechen konnten. Sonarpad normalisiert nun die Dauer der Quelldatei und der Gemini-Chunks nur dann, wenn dieses anomale Zeitstempelmuster erkannt wird; normale Videos und das bereits funktionierende Verhalten der Audiodeskription bleiben unverändert.
2. Das Ducking der Audiodeskription wurde für eine natürlichere Mischung verbessert. Der Originalton wird nun bereits vor Beginn der Stimme sanft abgesenkt und nach der Beschreibung allmählicher auf den normalen Pegel zurückgeführt; bei dicht aufeinanderfolgenden Beschreibungen bleibt der Hintergrundpegel stabil, sodass wiederholtes Absenken und Anheben vermieden wird.
3. Ein Problem wurde behoben, durch das der abschließende Export der Audiodeskription bei einigen AVI-Dateien und anderen Quellen mit 5.1-/Mehrkanalton fehlschlagen konnte, wenn die Dekodierung mit einem unvollständigen verschachtelten Audioframe endete. Sonarpad ergänzt nun ausschließlich diesen letzten Teilframe sicher mit der unbedingt erforderlichen Anzahl stiller Samples; bereits vollständige Frames sowie normale Mono-, Stereo- und Mehrkanaldateien bleiben unverändert.


Version 0.9.3 – 2026-09-03

SAPI5-Stimmen
1. Ein Problem wurde behoben, durch das einige lokale SAPI5-Stimmen bei aktiviertem Cursor-Folgen, beim Lesen mit mehreren Stimmen oder beim Erstellen von Hörbüchern/MP3-Dateien stumm bleiben konnten. Sonarpad verwendet nun einen SAPI5-Dateisyntheseweg, der unter Windows zuverlässig funktioniert und weiterhin abgebrochen werden kann; die normale direkte Wiedergabe bleibt unverändert.
2. Die Cursorposition bei der mehrstimmigen Dialogwiedergabe wurde korrigiert. Von Sonarpad automatisch für Dialoge eingefügte Sprach-Tags werden jetzt nur noch als Wiedergabemetadaten behandelt und nicht mehr als Zeichen im Editor, sodass der Cursor nach F4 oder F6 nicht mehr vor den tatsächlichen Text springt. Einsprachige Wiedergabe und ausdrücklich im Dokument vorhandene <voice>-Tags bleiben unverändert.

KI-Audiodeskription
1. Direkt hinter dem Feld für den Gemini-API-Schlüssel wurde das Kontrollkästchen „API-Schlüssel anzeigen“ hinzugefügt. Es ist standardmäßig deaktiviert; beim Aktivieren wird der vollständige Schlüssel vorübergehend sichtbar, damit geprüft werden kann, ob er vollständig eingefügt wurde. Beim erneuten Öffnen des Fensters ist der Schlüssel wieder verborgen.

Podcasts und Wikipedia
1. Nach dem Speichern einer Podcast-Aufnahme fragt Sonarpad jetzt, ob der Ordner mit der gespeicherten Datei geöffnet werden soll, wie bereits beim Speichern von YouTube-/Streaming-Medien.
2. Wird ein weiterer Wikipedia-Artikel in einen Editor importiert, der bereits Text enthält, wird der neue Artikel jetzt am Ende angefügt statt am Anfang eingefügt. Der Cursor wird an den Anfang des neu importierten Artikels gesetzt.


Version 0.9.2 – 2026-09-02

KI-Audiodeskription
1. Ein Problem wurde behoben, durch das die KI-Audiodeskription beim abschließenden MP3-Export von Videos mit Mehrkanalton wie 5.1 fehlschlagen konnte. Sonarpad mischt Mehrkanalton jetzt nur dann automatisch auf Stereo herunter, wenn dies für die MP3-Kodierung erforderlich ist; Mono- und Stereo-Exporte bleiben unverändert.
2. Wenn eine KI-Audiodeskription für ein Video mit mehreren Audiospuren gestartet wird, fragt Sonarpad jetzt vor der Verarbeitung, welche Spur verwendet werden soll. Die barrierefreie Auswahlliste lässt sich mit den Pfeiltasten ändern; OK startet die Audiodeskription mit der gewählten Spur, während Abbrechen das Audiodeskriptionsfenster schließt und den Fokus zum Sonarpad-Editor zurückbringt.

YouTube und Streaming
1. Ein Problem wurde behoben, durch das beim Starten der KI-Audiodeskription für ein Video auf Seite 2 oder einer späteren Seite einer YouTube-Playlist oder eines Kanals das YouTube-Auswahlfenster erneut geöffnet werden und dem Audiodeskriptionsfenster den Fokus entziehen konnte. Sonarpad schließt die Auswahl nun korrekt, ohne zu vorherigen Seiten zurückzukehren.
Version 0.9.1 – 2026-09-01

YouTube-Downloads
• Ein Problem wurde behoben, durch das Fortschrittsfenster von YouTube-/Streaming-Downloads nach dem Wechsel zu einer anderen Anwendung mit Alt+Tab wiederholt in den Vordergrund zurückkehren konnten. Downloads laufen jetzt im Hintergrund weiter, ohne den Fokus zu stehlen.
• Die Barrierefreiheit der Downloadanzeige wurde verbessert. Beim Zurückkehren zum Fortschrittsfenster können Screenreader den aktuellen Status und den Prozentsatz lesen. Bei Playlists nennt Sonarpad außerdem die Nummer des aktuellen Elements, die Gesamtzahl der Elemente und den Titel.
• Falsche Hang-Meldungen des Watchdogs bei langen Downloads und Konvertierungen wurden behoben, wenn das Fortschrittsfenster weiterhin reagierte.
• Für Playlist-Downloads wurde ein Kombinationsfeld „Format“ hinzugefügt. In der Videoliste kann mit Tab MP4, MP3, M4A, OPUS, OGG, WAV oder FLAC ausgewählt werden, bevor der Mehrfachdownload gestartet wird.
• Das Speichern von Streaming-Medien wurde neu organisiert. Format und Qualität werden jetzt beim Speichern gewählt und nicht mehr im ersten Streaming-Suchfenster. „Medien speichern“ öffnet einen gemeinsamen Dialog für Format und Qualität; bei Playlist-Downloads stehen beide Kombinationsfelder zur Verfügung.

KI-Audiodeskription
• Ein Problem wurde behoben, durch das die KI-Audiodeskription bei einigen MKV-Videos nicht gestartet werden konnte. Sonarpad verarbeitet Videos mit unregelmäßigen oder fehlenden Zeitstempeln jetzt zuverlässiger.

Version 0.9.0 – 2026-08-31

KI-Audiodeskription — neue Hauptfunktion
• Unter Werkzeuge > Multimedia wurde „Audiodeskription mit KI erstellen“ hinzugefügt. Sonarpad analysiert den Ton, findet dialogfreie Stellen, erzeugt Beschreibungen mit Gemini und verwendet die bereits vorhandenen Sprachmodule, ohne über Dialoge zu sprechen.
• Die Synchronisation zwischen dem Geschehen im Video und den Beschreibungen wurde verbessert; von Gemini erzeugte Zeitangaben werden automatisch geprüft.
• „Erweiterte Pausen aktivieren“ ist standardmäßig deaktiviert. Die Option kann bei dialogreichen Inhalten oder wenig verfügbarem Platz aktiviert werden, damit längere Beschreibungen eingefügt werden können.
• Sonarpad kann versuchen, Figuren zu erkennen und ihre Namen zu verwenden. Figurenkataloge können über mehrere Folgen einer Serie hinweg beibehalten werden, um die Kontinuität zu verbessern.
• Projekte können gespeichert, später bearbeitet und erneut exportiert werden, ohne alles noch einmal mit Gemini erzeugen zu müssen.
• Wird der Vorgang unterbrochen, speichert Sonarpad den Fortschritt und kann die Audiodeskription fortsetzen. Ist das Gemini-Kontingent erschöpft, kann gewartet, das Modell gewechselt oder beendet werden, ohne bereits fertige Arbeit zu verlieren.
• Im Fenster lassen sich Sprache, Detailgrad, Gemini-Modell, Sprachmodul und Stimme auswählen; die verwendeten Einstellungen werden gespeichert.
• Das Modul ist in allen 17 Sonarpad-Sprachen verfügbar. Während der Erstellung zeigt die Oberfläche nur Fortschritt, aktuellen Status und Abbrechen; anschließend kann die MP3 direkt im internen Player geöffnet werden.

E-Books und Dokumente
• Import von DRM-freien Kindle-Dateien in MOBI, AZW und AZW3 hinzugefügt; Text und Kapitel stehen im Editor und im Index zur Verfügung.
• Unterstützung für DAISY 2.02 und DAISY 3 hinzugefügt. DAISY-Hörbücher verwenden Sonarpads internen Player und beachten Kapitelnavigation und Wiedergabegrenzen.
• Kindle- und DAISY-Dateien werden importiert, ohne die Originaldatei zu überschreiben; DRM-geschützte Kindle-Bücher werden ausdrücklich abgelehnt.
• „Speichern unter“ für EPUB wurde korrigiert: Bei Auswahl von TXT oder einem anderen Format wird nun die gewählte Dateiendung verwendet, während das ursprüngliche EPUB mit dem geöffneten Dokument verknüpft bleibt.

RSS und Artikel
• Mehrfachauswahl für RSS-Artikel hinzugefügt, damit mehrere Artikel in einem Vorgang gelöscht werden können.
• RSS unterstützt jetzt echte Ordner, die beim OPML-Import und -Export einschließlich leerer Ordner erhalten bleiben.
• Feeds können innerhalb des aktuellen Ordners mit Nach oben, Nach unten, An den Anfang, Ans Ende und An Position neu angeordnet werden.

Barrierefreiheit, Anleitungen und Oberfläche
• Die Sonarpad-Anleitungen wurden mit einem Inhaltsverzeichnis neu geordnet; außerdem wurde eine vollständige Anleitung zur KI-Audiodeskription hinzugefügt.
• Ein Problem der deutschen Übersetzung wurde behoben, durch das Öffnen, Speichern unter und andere Dateiauswahldialoge nicht erscheinen konnten.

Stimmen und Sprachen
• Der herunterladbare Google-TTS-Katalog wurde von 104 auf 156 Pakete und von 53 auf 81 Sprachvarianten erweitert.
• Neue Google-TTS-Pakete und lokalisierte Namen weiterer Sprachen wurden in der gesamten Oberfläche ergänzt.

Version 0.8.4 – 2026-07-24

Bearbeiten von EPUB-Dokumenten
• Sonarpad kann EPUB-Dokumente jetzt nicht nur öffnen, sondern auch bearbeiten und erneut im EPUB-Format speichern. Dabei bleiben die ursprüngliche Formatierung, das Inhaltsverzeichnis, Fußnoten, Bilder, Stylesheets, Metadaten und interne Verknüpfungen erhalten.
• Das EPUB-Format steht unter „Speichern unter“ für Dokumente zur Verfügung, die aus einer EPUB-Datei geöffnet wurden. Beim Speichern wird nur der geänderte Text aktualisiert, während die Buchstruktur unverändert bleibt.

Zuverlässigkeit von Audiobüchern
• Ein zeitweilig auftretendes Problem wurde behoben, bei dem eine Syntheseeinheit nach fünf fehlgeschlagenen Google-TTS-Versuchen stillschweigend verworfen wurde, sodass im fertigen Audiobuch ein Textabschnitt fehlen konnte.
• Google-Einheiten werden nun wiederholt, bis sie erfolgreich sind oder der Benutzer abbricht. Der Start der Prozesse wird zeitlich versetzt, um vorübergehende Chrome- und Dateikonflikte zu verringern; außerdem beendet Sonarpad die Erstellung, anstatt ein Audiobuch mit fehlendem Segment zu speichern.
• Edge-Audiobücher wiederholen vorübergehende Netzwerk-, WebSocket-, Zeitüberschreitungs-, Dienstbegrenzungs- und ungültige Audiofehler jetzt ohne feste Obergrenze, bis die Synthese gelingt oder der Benutzer abbricht; dies gilt auch für gemischte Stimmen und zeitbasierte Aufteilungen. SAPI4 und SAPI5 behalten adaptive, begrenzte Wiederholungen bei; schlägt ein Segment weiterhin fehl, beendet Sonarpad den Vorgang, ohne ein unvollständiges Audiobuch zu speichern.

Navigation in digitalen Bibliotheken
• Die Suchergebnisse von LibriVox, Internet Archive und Project Gutenberg verwenden jetzt wie YouTube eine Seitennavigation: „Zu den vorherigen Ergebnissen“ steht am Anfang und „Zu den nächsten Ergebnissen“ am Ende der Liste.
• Die Fokusübergänge in LibriVox wurden korrigiert: Beim Öffnen eines Buches oder Kapitels springt der NVDA-Fokus nicht mehr in den Haupteditor, bevor die nächste Liste oder der Player geöffnet wird.
• Während der Suche und beim Laden von LibriVox-Büchern schützt nun ein lokalisiertes Ladefenster den Fokus und bleibt während der gesamten Anfrage im Vordergrund. Dadurch springt der NVDA-Fokus nicht mehr zur Eingabeaufforderung, zu Windows Terminal oder zu einer anderen Anwendung.

YouTube-Playlist-Downloads
• YouTube-Playlists verfügen jetzt über einen zugänglichen Mehrfachauswahl-Befehl, mit dem ausgewählt werden kann, welche Videos heruntergeladen werden sollen, ohne den vorhandenen Befehl „Medien speichern“ für das aktuell wiedergegebene Element zu ändern.
• Die ausgewählten Elemente werden nacheinander mit dem beim Öffnen der Playlist gewählten Format und der gewählten Qualität heruntergeladen, entsprechend der Playlist-Reihenfolge nummeriert und in einem eigenen Ordner innerhalb des konfigurierten Medienordners gespeichert.
• Das Auswahlfenster enthält „Alle auswählen“ und „Auswahl aufheben“, meldet die Anzahl der ausgewählten Elemente, erlaubt einen Abbruch unter Beibehaltung bereits fertiger Dateien und zeigt nicht heruntergeladene Elemente deutlich an.
• Die Playlist-Einträge sind jetzt native Kontrollkästchen: Screenreader geben Titel, Steuerelementtyp und Aktivierungsstatus automatisch aus, ohne zusätzliche Auswahlwörter im sichtbaren Titel oder erzwungene Sprachausgaben.

Version 0.8.3 – 2026-07-23

Dunkler Modus
• Ein dunkler Modus wurde hinzugefügt. Er kann über das Menü Ansicht aktiviert werden und wird in den Benutzereinstellungen gespeichert.
• Das dunkle Design wird auf den Editor, die Menüs, Nebenfenster und die wichtigsten Steuerelemente angewendet. Die Textfarben werden angepasst, damit Lesbarkeit und Barrierefreiheit erhalten bleiben.

Deutsche Übersetzung
• Deutsch wurde als vollständige Sprache der Benutzeroberfläche hinzugefügt und kann in den Optionen ausgewählt werden.
• Alle Oberflächentexte, Nachrichten- und RSS-Funktionen, die Rechtschreibprüfung, der Kalender, die Spendeninformationen, das Handbuch und das Änderungsprotokoll sind auf Deutsch verfügbar.
• Der Kalender enthält deutsche Bezeichnungen für alle 365 Tage sowie sämtliche 128 Zitate des Tages auf Deutsch.
• Für Nachrichten und RSS wurden deutsche Standardquellen hinzugefügt; die deutsche Rechtschreibprüfung verwendet das Windows-Wörterbuch de-DE.

Sprachausgabe
• Folgen aus drei oder mehr Punkten werden für SAPI5, SAPI4 und Google TTS nicht mehr als einzelne Satzzeichen beziehungsweise „Punkt Punkt“ vorgelesen.
• Satzzeichenfolgen werden vor der Synthese bereinigt, ohne normale Satzpausen oder zwei aufeinanderfolgende Punkte zu verändern.

Brasilianisches Portugiesisch und Google News
• Brasilianisches Portugiesisch wurde als vollständige Oberflächensprache hinzugefügt. Es ist von Portugiesisch (Portugal) getrennt und kann in den Optionen ausgewählt werden.
• Die vollständige Oberfläche, der Kalender mit allen Zitaten, die Rechtschreibprüfung, Spendeninformationen, das Handbuch und das Änderungsprotokoll sind auf brasilianischem Portugiesisch verfügbar.
• Google News unterstützt nun die brasilianische Lokalisierung, brasilianische Kategorien und getrennte brasilianische Standard-RSS-Quellen.
• Wenn der Feed sie bereitstellt, werden weitere Quellen zum selben Thema als barrierefrei zugängliche Untereinträge in der Baumansicht angezeigt.

LibriVox
• Die LibriVox-Suche wurde optimiert, um übermäßige Anfragen an den Dienst und ein Blockieren der Benutzeroberfläche zu vermeiden. Umfangreiche Katalogscans wurden entfernt, die Anzahl der Versuche reduziert und kürzere Zeitlimits eingeführt.

Sprachausgabe
• Folgen aus drei oder mehr Punkten werden jetzt vor dem Vorlesen normalisiert. Dadurch wird verhindert, dass einige Stimmen „Punkt Punkt“ aussprechen oder Abschnitte erzeugen, die nur aus Satzzeichen bestehen.

Verwandte Google-News-Artikel
• Für jede Nachricht werden jetzt, sofern verfügbar, verwandte Artikel angezeigt, also weitere Artikel zum selben Ereignis. Um sie zu lesen, genügt es, den Hauptartikel zu erweitern, wenn Sonarpad meldet, dass verwandte Artikel verfügbar sind. Wer diesen Bereich nicht erweitern möchte, kann beim Hauptartikel einfach die Eingabetaste drücken und die Nachricht wie gewohnt lesen.
• Verwandte Artikel verwenden jetzt dasselbe Gelesen/Ungelesen-System wie Hauptartikel, einschließlich zugänglicher Ansagen, Datum und Uhrzeit, Speicherung des Status sowie dessen Beibehaltung nach einer Aktualisierung der Quellen oder einem Neustart von Sonarpad.

Ansagen in Audiobuchteilen
• In den Audiooptionen wurde das Kombinationsfeld „Ansage am Anfang jedes Teils“ hinzugefügt. Bei in mehrere Dateien aufgeteilten Audiobüchern kann jeder Teil ohne Ansage oder mit Buchtitel, Buchtitel und Teilenummer, Dateiname oder Dateiname und Teilenummer beginnen.

Version 0.8.2 – 2026-07-17

Digitale Bibliotheken und Hörbücher
• Project Gutenberg wurde mit Suche nach Titel oder Autor und einer Sprachauswahl hinzugefügt.
• EPUB-Bücher von Project Gutenberg werden nach Dokumente\Sonarpad\Documents heruntergeladen. Nach dem Download fragt Sonarpad, ob das Buch sofort im Editor geöffnet werden soll.
• Internet Archive wurde zum Suchen und Anhören von Audiosammlungen hinzugefügt, darunter historische Radiosendungen, Reden und Live-Musik.
• LibriVox wurde zum Suchen von Hörbüchern nach Titel oder Autor hinzugefügt. Kapitel lassen sich direkt mit demselben Player wiedergeben, der auch für Podcasts verwendet wird.
• Die drei neuen Funktionen befinden sich im Menü Werkzeuge und bei aktivierter Menügruppierung im Bereich Lesen.

Lange Audiotranskriptionen
• Die Transkription langer Audiodateien wurde korrigiert: Das Audio wird automatisch in 15-minütige Abschnitte geteilt, abschnittsweise transkribiert und anschließend wieder zusammengeführt. Dadurch werden Fehler bei langen Aufnahmen vermieden.

YouTube
• Die wichtigsten Aktionen, die zuvor erst nach dem Öffnen eines YouTube-Videos über das Wiedergabe-Menü erreichbar waren, stehen jetzt auch direkt im Kontextmenü desselben Videos zur Verfügung, zum Beispiel „Aktuelles Audio transkribieren“, „Audiodeskription mit KI erstellen“ und „Medium speichern“, für eine einfachere Bedienung.
• Der Befehl „Link kopieren“ wurde hinzugefügt. Er ist auch mit Strg+C verfügbar und kopiert die URL des ausgewählten YouTube-Videos, der Playlist oder des Kanals in die Zwischenablage.

Version 0.8.1 – 2026-07-16

Google-Sprachausgabe
• Der Start von Google TTS wurde auf Windows-Systemen korrigiert, auf denen vom internen Browser-Server akzeptierte Verbindungen den nicht blockierenden Socketmodus übernahmen. Dies verursachte Fehler 10035 und verhinderte die Ausgabe heruntergeladener Stimmen.
• Sonarpad wartet jetzt, bis die WASM-Engine von Chrome oder Edge vollständig geladen ist, bevor eine Stimmvorschau oder das Lesen mit F5 beginnt. Dadurch wird die Meldung „Chrome WASM TTS engine was not loaded“ vermieden.
• Im ausgeblendeten Browser werden die Seitenübersetzung und die Renderer-Barrierefreiheit deaktiviert, sodass weder „Seite übersetzen“ angesagt wird noch Lesebefehle gestört werden.
• Im Bereich „Stimmen im Editor“ erscheint bei ausgewählter Google-Engine die Schaltfläche „Google-Stimmen verwalten...“. Nach dem Schließen der Verwaltung wird die Liste installierter Stimmen sofort aktualisiert.
• Warnungen zu Abhängigkeiten beim Entfernen von Google-Stimmpaketen sind nun in allen Oberflächensprachen lokalisiert.

Aktualisierungserlebnis
• Nach einer automatischen Aktualisierung öffnet sich das Abschlussfenster mit dem Änderungsprotokoll erst nach der anfänglichen Wiederherstellung des Editorfokus und bleibt im Vordergrund, statt erst nach dem Drücken von Tab sichtbar zu werden.

PDF-Dokumente
• PDF-Dateien wurden korrigiert, deren eingebetteter Text NUL-Zeichen enthielt und beim Laden in den Editor an der ersten Stelle abgeschnitten wurde.
• Wenn pdf-extract eingebettete NUL-Zeichen zurückgibt, versucht Sonarpad die Extraktion erneut mit PDFium. Verbleibende NUL-Zeichen werden entfernt, bevor der Text an Windows-Steuerelemente gesendet wird, sodass der restliche Dokumentinhalt erhalten bleibt.

Barrierefreiheit der Menüs
• Die Erzeugung von Zugriffstasten zur Laufzeit wurde entfernt. Zugriffstasten sind jetzt ausdrücklich in jeder der 15 Oberflächenübersetzungen eingetragen und bleiben bei jedem Start gleich.
• Alle stabilen Hauptmenüeinträge und Untermenüs wurden geprüft, darunter Wiedergabe, Schriftarten, Bild speichern und EPUB-Inhaltsverzeichnis anzeigen. Fehlende oder doppelte Zugriffstasten innerhalb desselben Menüs wurden direkt in den Übersetzungen korrigiert.
• Automatische Tests prüfen nur noch die Übersetzungen und schlagen fehl, wenn eine Zugriffstaste fehlt, ungültig oder doppelt ist. Menübeschriftungen werden zur Laufzeit nicht mehr verändert.
• In außergewöhnlich großen Menüs, in denen der übersetzte Text nicht genügend unterschiedliche Zeichen bietet, wird eine ausdrückliche numerische Zugriffstaste in der Windows-Standardform „(&1)“ angezeigt.

Version 0.8.0 – 2026-07-15

Online-Wörterbuch
• Deutsch wurde dem Online-Wörterbuch Wiktionary hinzugefügt.
• Deutsche Definitionen und Synonyme werden anhand der Struktur des deutschen Wiktionary ausgewertet, statt Deutsch lediglich in der Auswahlliste anzubieten.

Zuverlässigkeit von SAPI5-Hörbüchern
• Bei der Hörbucherstellung mit SAPI5 werden bis zu 12 parallele Arbeitsprozesse beibehalten, wenn die ausgewählte Stimme zuverlässige Ergebnisse erzeugt.
• Jeder erzeugte Teil wird anhand von Dateigröße, geschätzter Dauer und einem vorsichtigen Vergleich mit dem zugewiesenen Text geprüft.
• Fehlende oder verdächtige Teile werden automatisch mit schrittweise verringerter Parallelität neu erzeugt: 12, 8, 6, 4, 2 und schließlich 1 Arbeitsprozess. Nur problematische Teile werden wiederholt.
• Die zuverlässige Höchstzahl der Arbeitsprozesse wird für jede SAPI5-Stimme getrennt gespeichert. Stimmen, die mit 12 Prozessen korrekt arbeiten, werden dadurch nicht verlangsamt.
• Eine abschließende Integritätsprüfung verhindert, dass Sonarpad unbemerkt eine MP3-Datei akzeptiert, die wesentlich kürzer als die erzeugten Teile ist.
• Ausführliche Diagnosedaten werden in `sapi5_audiobook_diagnostic.log` geschrieben.
• Jede SAPI5-Syntheseeinheit läuft nun in einem eigenen ausgeblendeten Sonarpad-Prozess. Wenn eine Drittanbieter-Stimme abstürzt, wird nur dieser Arbeitsprozess beendet; die Hauptanwendung bleibt geöffnet.
• Während derselben Hörbucherstellung werden unvollständige Teile sofort mit der nächstniedrigeren Parallelitätsstufe erneut versucht. Bereits geprüfte Teile bleiben erhalten.
• Die Wiederherstellung beim nächsten Programmstart bleibt nur als zusätzliche Absicherung bestehen, falls die Hauptanwendung oder der Computer unterbrochen wird.

SAPI4-Arbeitsprozesse für Hörbücher
• Die vom Benutzer ausgewählte Anzahl der SAPI4-Prozesse wird nun bis zu einer technischen Obergrenze von 64 berücksichtigt. Die frühere verdeckte Grenze von 16 wurde entfernt.
• Die tatsächliche Anzahl wird nur dann verringert, wenn das Hörbuch weniger Arbeitseinheiten enthält als angefordert.
• Wenn ein oder mehrere SAPI4-Bridge-Prozesse fehlschlagen, bleiben abgeschlossene Teile erhalten. Nur fehlgeschlagene Einheiten werden automatisch mit schrittweise geringerer Parallelität wiederholt.
• Sonarpad prüft nun den Beendigungsstatus der SAPI4-Bridge und weist leere oder ungültige Audioteile zurück, statt sie als erfolgreich zu behandeln.

Proxy-Konfiguration
• In den Netzwerkeinstellungen wurde ein eigenes Feld für den Proxy-Port hinzugefügt.
• Der Port kann unabhängig von der Proxy-Adresse eingegeben werden, wird auf den Bereich 1 bis 65535 geprüft und ersetzt korrekt einen bereits in der URL enthaltenen Port.

Radiosuche nach Sprache und Land
• Die Filter Sprache und Land werden nun mit sämtlichen verfügbaren Einträgen des Radio-Browser-Verzeichnisses aktualisiert, statt auf eine feste Liste beschränkt zu sein.
• Sprachnamen werden auch erkannt, wenn Radio Browser sie in einer anderen Schrift, als Eigenbezeichnung, Abkürzung oder Kombination mehrerer Sprachen liefert. Sie werden in der aktuellen Oberflächensprache angezeigt. Werte, die keine echten Sprachen sind, etwa Zahlen, Genres, Länder oder allgemeine Bezeichnungen, werden herausgefiltert.
• Das Verzeichnis wird im Hintergrund aktualisiert. Eine Ersatzliste bleibt verfügbar, wenn Radio Browser nicht erreichbar ist.
• Doppelte Radio-Browser-Sprachen, die nach der Übersetzung gleich lauten, werden zu einem einzigen Kombinationsfeldeintrag zusammengeführt. Dadurch werden stumme Schritte mit Screenreadern vermieden.

Wichtige Verbesserung: Synchronisierung von Sprachausgabe und Cursor
• Die Synchronisierung zwischen Sprachausgabe und Cursorbewegung wurde für alle unterstützten Sprach-Engines deutlich verbessert.
• Wenn „Cursor beim Lesen bewegen“ aktiviert ist, verwendet Sonarpad ein gemeinsames Fortschrittssystem für Microsoft Edge Neural, Google TTS, SAPI4, SAPI5 und OneCore.
• Der Cursor folgt dem tatsächlich gesprochenen Text genauer; Sätze und Textabschnitte werden einheitlicher unterteilt.
• Verfrühte Bewegungen, Verzögerungen, unregelmäßige Sprünge und Unterschiede zwischen den Engines wurden erheblich reduziert.
• Nach Pause, Fortsetzen, einer Suche im Dokument oder dem Wechsel der Sprach-Engine bleibt die richtige Position zuverlässiger erhalten.

Getrennte Spuren bei Podcast-Aufnahmen
• „Mikrofon und System- oder Anwendungsaudio in getrennten Dateien speichern“ wurde hinzugefügt.
• Wenn Mikrofon und eine weitere Quelle gemeinsam aufgenommen werden, kann Sonarpad eine reine Mikrofonspur und eine zweite Datei mit Systemaudio, einer Anwendung oder den ausgewählten Anwendungen erzeugen.
• Die getrennte Aufnahme ist sowohl in MP3 als auch WAV verfügbar.
• Ist die Option deaktiviert, erstellt Sonarpad weiterhin eine normal gemischte Datei.
• Getrennte Dateien erleichtern Lautstärkeanpassung, Rauschminderung und spätere Bearbeitung von Podcasts, Interviews und Anleitungen.

Geplante Radioaufnahmen
• Radioaufnahmen können im Voraus geplant werden.
• Für jede Aufnahme lassen sich Sender, Tag, Startstunde und -minute sowie Dauer auswählen.
• Eine benutzerdefinierte Dauer von 1 bis 1.440 Minuten ist möglich.
• Aufnahmen können einmalig, täglich oder wöchentlich ausgeführt werden.
• Das Aufnahmefenster zeigt aktive und geplante Aufnahmen, vorgesehenes Datum und Uhrzeit, Dauer sowie verbleibende Zeit bis zum Start deutlicher an.
• Geplante Aufnahmen können die Windows-Aufgabenplanung verwenden und dadurch auch starten, wenn Sonarpad noch nicht geöffnet ist.

Kalender
• Ein vollständig per Tastatur zugänglicher Kalender wurde hinzugefügt.
• Benutzer können zum vorherigen oder nächsten Tag wechseln, schnell zu Heute zurückkehren und Feiertage sowie Gedenktage prüfen.
• Namenstag und Zitat des Tages wurden hinzugefügt; beide können gelesen, gesprochen oder kopiert werden.
• Erinnerungen lassen sich erstellen, bearbeiten, löschen, verschieben und als erledigt markieren.
• Hinweise können genau zum Termin oder im Voraus erscheinen und die Windows-Aufgabenplanung verwenden, auch wenn Sonarpad geschlossen ist.

Wetter
• Ein Bereich für Wettervorhersagen wurde hinzugefügt.
• Benutzer können nach einer Stadt suchen und zuletzt betrachtete Orte schnell erneut öffnen.
• Verfügbar sind aktuelle Bedingungen, Temperatur, Mindest- und Höchstwerte, Luftfeuchtigkeit, Niederschlagswahrscheinlichkeit und Vorhersagen für die folgenden Tage.
• Temperaturen können in Celsius, Fahrenheit oder automatisch angezeigt werden.

Kinofilme
• Ein Bereich für aktuell im Kino laufende und demnächst erscheinende Filme wurde hinzugefügt.
• Titelsuche, Handlung, Erscheinungsdatum und Wiedergabe des Trailers stehen zur Verfügung.

Google-Sprachausgabe
• Google TTS wurde zum Lesen von Dokumenten und Erstellen von Hörbüchern hinzugefügt.
• Eine Stimmenverwaltung listet Stimmen auf, filtert sie nach Sprache, lädt sie herunter und entfernt nicht mehr benötigte Stimmen.
• Geschwindigkeit, Lautstärke und Tonhöhe können eingestellt werden.
• Die Tonhöhe von Google-Natural-Stimmen wird für ein natürlicheres und stabileres Ergebnis direkt von der Engine angewendet.
• Reaktionsfähigkeit und Zuverlässigkeit von Google TTS wurden verbessert; Synthese-Zeitlimits passen sich der gewählten Sprechgeschwindigkeit an.
• Unnötige Wartezeiten bei ausbleibender Antwort der Engine wurden verringert, und die Behandlung von Fehlern und Unterbrechungen wurde verbessert.
• Die Diagnoseprotokollierung ist bei gleichzeitigen Vorgängen stabiler.

EPUB-Inhaltsverzeichnis
• Sonarpad erkennt nun das in EPUB-Büchern eingebettete Inhaltsverzeichnis.
• Sein Vorhandensein wird angesagt; es kann über das Menü Ansicht geöffnet werden.
• Kapitel und Unterkapitel werden hierarchisch dargestellt.
• Mit Eingabe wird sofort zur ausgewählten Stelle im Buch gewechselt.

Nachrichten und RSS-Quellen
• Der Nachrichtenbereich wurde um neue Such- und Organisationswerkzeuge erweitert.
• Eine Auswahl der Nachrichtensprache wurde hinzugefügt.
• Benutzer können innerhalb der RSS-Quellen suchen und Nachrichten aus ihrer Stadt lesen.
• RSS-Quellen der Gemeinschaft können durchsucht, zur eigenen Sammlung hinzugefügt und der Sonarpad-Gemeinschaft vorgeschlagen werden.

Podcast-Aufnahme
• Aufgenommen werden können nur das Mikrofon, das gesamte Systemaudio, eine Anwendung, mehrere ausgewählte Anwendungen oder Mikrofon und Anwendungen gemeinsam.
• Mikrofon und Audioquelle können ausgewählt, Lautstärken getrennt angepasst und Pegel in Echtzeit überwacht werden.
• Pause und Fortsetzen, Ausgabe als MP3 oder WAV, Auswahl der MP3-Bitrate und des Zielordners wurden hinzugefügt.
• Der Computer kann während der Aufnahme wach gehalten werden.
• Getrennte Dateien erhalten eindeutige Namen, sodass die Mikrofonspur sofort von System- oder Anwendungsaudio unterschieden werden kann.

Radio
• Der Radiobereich wurde umfassend neu geordnet.
• Sender können nach Name oder Freitext, Sprache, Land, Stadt, Musikgenre oder Kategorie gesucht werden.
• Die Favoritenverwaltung wurde verbessert; alle Filter lassen sich schnell zurücksetzen.
• Sender können der Sonarpad-Gemeinschaft vorgeschlagen werden.
• Live-Aufnahme, „Aufnehmen und wiedergeben“, eine Aufnahmeliste sowie Löschen und Verwalten von Aufnahmen wurden hinzugefügt.
• Radioaufnahmen werden in einem eigenen Unterordner des Hauptaufnahmeordners gespeichert.

Medienwiedergabe
• Die Stabilität des Medienplayers wurde deutlich verbessert.
• Ein Problem, das mpv blockieren konnte, wurde behoben; die Kommunikation mit dem Player ist zuverlässiger.
• Das Öffnen verschiedener Mediendateitypen wurde verbessert.
• Sonarpad merkt sich nun die bei der Wiedergabe verwendete Lautstärke.
• Die Behandlung von Streams und Aufnahmen wurde verbessert.
• Dateien, die über Doppelklick oder „Öffnen mit“ aus Windows gestartet werden, wurden korrigiert.

PDF-Dokumente
• Die Erkennung von Formularfeldern in PDF-Dokumenten wurde hinzugefügt.
• Sonarpad kann ausfüllbare Felder finden, in einer zugänglichen Textform darstellen, ihre Werte bearbeiten lassen und die eingegebenen Daten wieder in die PDF-Datei speichern.
• Die Berechnung der Cursorposition während der Sprachausgabe wurde besonders für Dokumente mit Mehrbytezeichen oder komplexen Strukturen korrigiert.
• Das neue gemeinsame Synchronisierungssystem verbessert die Cursorbewegung zusätzlich für jede Sprach-Engine.

Barrierefreiheit und Tastaturbefehle
• Standardbearbeitungsbefehle wurden im gesamten Programm verbessert.
• Kopieren, Ausschneiden, Einfügen, Alles auswählen, Rückgängig und Wiederholen werden nun korrekt an das fokussierte Feld gesendet, auch in Nebenfenstern und Dialogen.
• Ein Problem, das die korrekte Aktualisierung von Braillezeilen verhindern konnte, wurde behoben.
• Die Fokusbehandlung in Nebenfenstern wurde verbessert.
• Die Sprachauswahl im Wikipedia-Fenster wurde korrigiert.
• Eine Option zum Gruppieren der Funktionen im Menü Werkzeuge nach Kategorien wurde hinzugefügt.
• Konfigurierbare Aktionen zum schnellen Öffnen von Kalender, Wetter und Kinofilmen wurden hinzugefügt.
• Die Darstellung des Änderungsprotokolls nach einer Aktualisierung wurde verbessert.

Hörbücher
• Die Hörbucherstellung bei geöffneten Dialogen oder anderen modalen Fenstern wurde verbessert.
• Die Fortschrittsbehandlung ist robuster und ignoriert veraltete Audioaktualisierungen. Dadurch werden Einfrieren, falsche Meldungen und nicht reagierende Fenster reduziert.
• Google TTS kann mit Einstellungen für Geschwindigkeit, Lautstärke und Tonhöhe auch zur Hörbucherstellung verwendet werden.

Künstliche Intelligenz
• Das standardmäßige Gemini-Modell wurde auf `gemini-3.5-flash` aktualisiert.

Allgemeine Korrekturen
• Mehrere Blockierungen bei der mpv-Wiedergabe wurden behoben.
• Das Öffnen einiger Audio- und Videodateien wurde korrigiert.
• Die an den Medienplayer gesendeten Befehle wurden verbessert.
• Die Wiederherstellung des Cursors während der Sprachausgabe wurde korrigiert.
• Tastenkürzel in Textfeldern von Hilfsfenstern wurden korrigiert.
• Die Stabilität der Hörbucherstellung wurde verbessert.
• Über Windows extern geöffnete Dateien wurden korrigiert.
• Die allgemeine Behandlung von Medien, RSS, Radio und EPUB wurde verbessert.

Version 0.7.1 – 2026-05-13

Neue Funktionen und Verbesserungen
• Die offizielle Website sonarpad.com wurde erstellt. Sie dient als zentraler Ort für Neuigkeiten, den Download der neuesten Programmversion, Besucherkommentare und künftig alle Sonarpad-Podcasts. Das Hilfemenü enthält nun „sonarpad.com besuchen“ zum schnellen Öffnen der Website.
• Ein Fehler wurde behoben, durch den Dateien mit Akzenten oder Sonderzeichen beim Start einer Sprachtranskription eine Fehlermeldung verursachten.
• Einträge wie Zeilenumbruch und Video während der Wiedergabe anzeigen zeigen im Menü Ansicht nun stets den korrekten aktivierten oder deaktivierten Zustand.
• Die YouTube-Suche wurde verbessert; mit Esc kann zur vorherigen Seite oder Ansicht zurückgekehrt werden.
• Vor der Wiedergabe wird geprüft, ob ein Video abgespielt werden kann. Sonarpad kann nun auch als Mix gekennzeichnete Videos oder Playlists wiedergeben, die zuvor nicht funktionierten.
• Die automatische Lesezeichenverwaltung wurde verbessert. Wurde sie aktiviert und später deaktiviert, blieben die Lesezeichen zuvor wirksam; nun ignoriert Sonarpad sie korrekt, bis die Option erneut aktiviert wird. Am Ende einer Mediendatei wird das Lesezeichen automatisch gelöscht.
• Die Behandlung von Sprach-Tags bei aktivierten Dialogen wurde verbessert. Beide Funktionen können nun gemeinsam verwendet werden, sodass Tags auch bei aktivierter Dialogoption eingefügt werden.
• Spracheinstellungen wurden klar nach Engine getrennt. Stimmprofile behalten ihre Einstellungen nun korrekt getrennt für Edge, SAPI5 und SAPI4.
• Ein Tag zum Einfügen von Pausen wurde in den Optionen und im Stimmbereich hinzugefügt, der mit Tab aus dem Editor erreichbar ist. Zur Auswahl stehen 250 ms, 500 ms, 1 Sekunde, 2 Sekunden oder eine benutzerdefinierte Dauer.
• Beim Abspielen eines YouTube-Videos und Starten einer Transkription wird nach der Rückkehr mit Alt+Tab nun korrekt die Schaltfläche Abbrechen der laufenden Transkription fokussiert.
• Fertige Transkriptionen werden automatisch gespeichert.
• Der Wikipedia-Import wurde verbessert. Es kann wahlweise nur ein Abschnitt gelesen und mit Esc aus dem Artikel zur Suche zurückgekehrt oder der gesamte Artikel importiert werden. Auch die Wikipedia-Sprache ist auswählbar.
• Ein weltweiter Radiobereich wurde hinzugefügt. Sender können nach Land, Sprache und Genre gesucht, lokale Sender der Sonarpad-Datenbank vorgeschlagen und als Favoriten gespeichert werden.
• Ein Routenbereich wurde hinzugefügt. Als Fortbewegungsart stehen zu Fuß, Fahrrad, Auto oder Rollstuhl zur Verfügung; außerdem kann zwischen kürzester und schnellster Route sowie der Anzeige durchquerter Gemeinden gewählt werden. Nach dem Import lässt sich die Karte über Datei > Bild speichern sichern.
• Drucken wurde dem Menü Datei hinzugefügt. TXT-Dateien druckt Sonarpad selbst; für DOCX, PDF und ähnliche Formate wird das zugeordnete Programm verwendet, um das ursprüngliche Layout möglichst zu erhalten.
• Ein Übersetzungsdienst für jedes Dokument wurde dem Kontextmenü des Editors hinzugefügt. DeepL und Google Übersetzer können kostenlos ohne API-Schlüssel verwendet werden; mit einem Gemini-API-Schlüssel steht auch Gemini zur Verfügung.
• Im Übersetzungsmenü kann die Zielsprache ausgewählt werden. Das Menü ordnet sich nach Nutzung: Werden zuerst Englisch, dann Französisch und Italienisch gewählt, erscheinen diese drei Sprachen oben.
• Mit eingetragenem Gemini-API-Schlüssel steht im Kontextmenü außerdem „Text zusammenfassen“ für beliebige Artikel zur Verfügung.
• Im Wiedergabemenü erscheint während einer Mediendatei ein Befehl zum Teilen des aktuellen Mediums. MP3, MP4 und weitere Formate können nach Anzahl der Teile oder nach Dauer jedes Teils geteilt werden.

Version 0.7.0 – 2026-04-25

Neuigkeiten
• Die mpv-Wiedergabe für Streaming wurde hinzugefügt. Videos von YouTube und unterstützten Websites werden sofort abgespielt; bei Auswahl von Behalten werden sie wie zuvor heruntergeladen. Für die Transkription von Streams werden sie zunächst heruntergeladen. mpv öffnet außerdem lokale Videos und verarbeitet Untertitel, wodurch viele zuvor nur eingeschränkt unterstützte Formate besser funktionieren.
• Die Podcast-Aufnahme von Systemaudio wurde verbessert: Es kann das gesamte Systemaudio, eine einzelne Anwendung oder mehrere Anwendungen gleichzeitig aufgenommen werden. Das Mikrofon lässt sich unabhängig davon ein- oder ausschalten.
• Hindi wurde hinzugefügt, einschließlich Oberflächenübersetzung, RSS-Quellen, Änderungsprotokoll und Sonarpad-Handbuch.
• In der Registerkarte Editor wurde eine Option hinzugefügt, mit der der Cursor bei den Pfeiltasten Auf und Ab stets an den Zeilenanfang gesetzt wird.
• Im Menü „Audio konvertieren“ wurde M4B als Zielformat hinzugefügt.

Korrekturen
• F10 wechselt während der Textwiedergabe wieder korrekt zur nächsten bevorzugten Stimme.
• Wird während einer Podcast-Aufnahme ein anderes Dokument geschlossen, endet die laufende Aufnahme nicht mehr ebenfalls.
• Bei YouTube-Kommentaren aus „Streaming-Audio wiedergeben...“ lädt Sonarpad zunächst nur die ersten 50 Hauptkommentare, jeweils einschließlich aller Antworten, und bietet am Ende einen Eintrag zum Nachladen sämtlicher Kommentare.
• Lesezeichen werden für Textdokumente und Mediendateien nun nach Position statt nach Erstellungsreihenfolge angezeigt und verarbeitet. Ein Lesezeichen an derselben Position wird nicht doppelt angelegt.
• Im Lesezeichenmenü wurde eine Option zur automatischen Verwaltung hinzugefügt. Beim Schließen einer lokalen oder gestreamten Mediendatei speichert Sonarpad die erreichte Position und setzt dort beim nächsten Öffnen fort. Bei Textdateien wird die Cursorposition gespeichert; nach gestarteter Sprachausgabe wird der zuletzt gelesene Satz gespeichert und beim nächsten Mal genau dort fortgesetzt.
• Im Menü Ansicht wurde ein Eintrag zum Anzeigen des Videos lokaler oder gestreamter Dateien hinzugefügt. Das Video erscheint vergrößert; Steuerelemente bleiben ausgeblendet, bis Alt gedrückt oder die Maus an den oberen Fensterrand bewegt wird. Dadurch ist der Inhalt für sehbehinderte Benutzer größer und besser nutzbar.

Version 0.6.9 – 2026-04-08

Korrekturen
• Die Bedienung von „In Dateien suchen“ wurde verbessert: Beim Öffnen von „Ordner durchsuchen“ gelangt der Fokus direkt zur Ordnerliste; das Öffnen eines Ergebnisses mit Eingabe unterbricht keine Tastaturbefehle mehr; Esc kehrt zum zuvor ausgewählten Ergebnis zurück; und nach Alt+Tab gelangt der Fokus je nach Zustand zum Suchfeld oder zur Ergebnisliste.
• F5 begann bisher stets am Dokumentanfang. Jetzt startet die Wiedergabe an der aktuellen Cursorposition; Umschalt+F5 und Strg+F5 bleiben für vorherigen und nächsten Satz erhalten.
• Nach „Gehe zu Zeile“ konnte Esc den Fokus aus Sonarpad herausbewegen. Nun kehrt der Fokus korrekt in den Editor zurück.
• Die Option „Zeilenumbruch“ wird sofort auf bereits geöffnete Dokumente angewendet, statt erst nach erneutem Öffnen der Datei wirksam zu werden.

Version 0.6.8 – 2026-04-07

Neuigkeiten
• Im Wiedergabemenü wurde ein Befehl hinzugefügt, der beliebige Audio- oder Videodateien mit Whisper transkribiert. In den Optionen gibt es nun den Bereich „KI und Transkription“, in dem Modell, optionale CUDA-Unterstützung für NVIDIA-Grafikkarten, Beibehaltung der Originalsprache und Zeitstempel eingestellt werden können.
• Der neue Befehl „Aktuellen Ordner transkribieren“ transkribiert alle unterstützten Audiodateien im Ordner des geöffneten Mediums in ein gemeinsames Dokument. Er bietet eigenen Fortschritt, Anzeige der aktuellen Datei und Abbruch und kann auch mit Alt+Umschalt+C gestartet werden.
• Offline-Sprachdiktat wurde mit demselben Ablauf wie die Audiotranskription hinzugefügt. Standardmäßig startet und beendet Strg+Umschalt+Leertaste das Diktat; das Kürzel ist konfigurierbar. Ab der zweiten Verwendung ist es schneller, weil die Engine im Speicher bereit bleibt. Auf Computern mit weniger als 4 GB RAM werden Vorladen und Wiederverwendung automatisch deaktiviert.
• Eine standardmäßig deaktivierte Editoroption lässt Esc das Editorfenster schließen.
• Die Podcast-Suche verwendet standardmäßig „iTunes + Spreaker“ und entfernt doppelte Treffer, wenn derselbe Podcast auf beiden Plattformen gefunden wird.
• Apple-Podcast-Suche, Kategorien und Top-Podcasts nach Kategorie verwenden nun das ausgewählte Land des Podcast-Verzeichnisses. Unter Optionen > RSS/Podcast kann Automatisch für das Systemland oder ein anderes Land gewählt werden.
• Die Ergebnisgrenze für Apple-Podcast-Kategorien wurde erhöht. Beim ersten Öffnen werden weiterhin 50 Ergebnisse geladen; „Weitere Ergebnisse laden“ lädt bis zu 200 Gesamtergebnisse, die Apple-Höchstgrenze, und ermöglicht flüssiges Blättern durch weitere Seiten.
• Sonarpad ist nun mit einem Teil seiner Funktionen auch für Mac verfügbar. Projekt: https://github.com/Ambro86/Sonarpad-Mac

Verbesserungen
• Mehr als 50 Länder sind für das Podcast-Verzeichnis auswählbar, sodass deutlich mehr nationale Kataloge zur Verfügung stehen.
• „Streaming-Audio wiedergeben...“ kann YouTube nach einem beliebigen Text durchsuchen oder einen Link zu einem YouTube-Kanal beziehungsweise einer Playlist annehmen und deren Inhalte anzeigen.
• YouTube-Ergebnisse in „Streaming-Audio wiedergeben...“ zeigen Titel, Dauer, Kanal und Aufrufzahl übersichtlicher an.
• YouTube-Kommentare werden ebenfalls unterstützt: Sie lassen sich über das Kontextmenü öffnen, Antworten können gelesen und Kommentarzweige mit der Pfeiltaste Rechts erweitert werden.
• Favoriten für YouTube-Kanäle und -Playlists wurden hinzugefügt. Sie können in den Ergebnissen über das Kontextmenü gespeichert, direkt aus der Favoritenliste nach dem URL-/Suchfeld geöffnet und dort später wieder entfernt werden. In Suchergebnissen steht dieses Kontextmenü nur für Kanäle und Playlists zur Verfügung.
• Wenn eine Streaming-Website eine Anmeldung verlangt, kann „Streaming-Audio wiedergeben...“ Zugangsdaten abfragen. Benutzer können diese eingeben, für die Website speichern und später unter Optionen > Audio verwalten.
• Die Fokusbehandlung während „Streaming-Audio wiedergeben...“ wurde verbessert, sodass das Fortschrittsfenster bei Download und Konvertierung stabiler bleibt.
• Im Menü Stimme wurden „Vorheriger Satz“ und „Nächster Satz“ mit konfigurierbaren Kürzeln zur Navigation während der Textwiedergabe hinzugefügt.
• Das Standardkürzel für „Datei mit Interpreter ausführen“ ist nun Strg+Umschalt+F5, damit Umschalt+F5 standardmäßig für „Vorheriger Satz“ verwendet werden kann.
• Unter Optionen > Stimme können Stimmprofile hinzugefügt, umbenannt und gelöscht werden.
• Die Rückspulintervalle unter Optionen > Audio wurden um Werte von einer Sekunde bis zu zwei Stunden erweitert.
• Eine russische Übersetzung wurde dank Dmitriy hinzugefügt.
• Unter Optionen > Audio lässt sich das Namensformat von Hörbuchteilen wählen: „Titel + Nummer“, „Nur Nummer“ oder „Nummer + Titel“.
• RSS-Artikel können über das Kontextmenü einem eigenen Favoriten-Feed hinzugefügt werden.
• Der RSS-Favoriten-Feed kann gelöscht werden und wird beim nächsten Hinzufügen eines Favoriten automatisch neu erstellt.
• RSS-Quellen können mit Strg+Umschalt+Pfeil Auf beziehungsweise Strg+Umschalt+Pfeil Ab verschoben werden.
• Das RSS-Fenster enthält eine integrierte Artikelvorschau. Der Text kann direkt dort geprüft und mit Tab schnell erreicht werden, bevor der vollständige Artikel im Editor geöffnet wird.
• Wenn weitere Artikel vorhanden sind, erscheint am Ende eines Feeds ausdrücklich „Weitere Nachrichten laden“. Eingabe lädt den nächsten Block und setzt den Fokus auf den ersten neu geladenen Artikel.
• Beim Hinzufügen oder Bearbeiten eines Eintrags im Aussprachewörterbuch kann mit „Groß-/Kleinschreibung beachten“ festgelegt werden, ob die Ersetzung die Schreibweise berücksichtigt.

Fehlerbehebungen
• „Streaming-Audio wiedergeben...“ berücksichtigt nun die unter Optionen festgelegte Podcast-Cachegrenze; dieselbe Grenze gilt auch für die Wiedergabe von Audiodeskriptionen.
• Zitatblöcke werden beim Wikipedia-Import nun korrekt übernommen.
• Der Webseitenparser wurde für WordPress-Seiten verbessert, bei denen Listeneinträge und einige Abschnittsüberschriften fehlen konnten.
• „Gehe zu Zeile“ füllt das Feld nun mit der aktuellen Zeilennummer vor.
• OPML-Exporte von Podcasts und RSS werden nun von iTunes akzeptiert.
• Bestätigungen für erfolgreichen OPML-Import und -Export von RSS und Podcasts wurden lokalisiert.
• Ein Fehler wurde behoben, durch den die Auswahl eines YouTube-Kanals nach einer Textsuche das Programm scheinbar blockieren konnte, statt die Videos dieses Kanals zu öffnen.
• Die Liste geöffneter Dateien wird nun im Menü Fenster statt fälschlich im Hilfemenü angezeigt.
• Ein Streaming-Sonderfall wurde korrigiert, bei dem die Wiedergabe begann, der Dialog „Stream wird heruntergeladen“ jedoch offen blieb, wenn die heruntergeladene Datei bereits dem Zielformat entsprach.
• Wenn ein Stream bereits MP3 ist und eine ausdrückliche MP3-Bitrate wie 128 kbit/s gewählt wurde, wird er nun auf diese Bitrate neu codiert, statt die Konvertierung zu überspringen.
• Beim Schließen von Transkriptionsdokumenten wird nun nach dem Speichern gefragt. Der vorgeschlagene Dateiname übernimmt den Namen der transkribierten Mediendatei statt der ersten Textzeile.
• Alt+Umschalt+L öffnet während der Wiedergabe nun korrekt die Kapitelliste.
• Alt+Umschalt+T startet nun korrekt „Aktuelles Audio transkribieren“, statt das Menü Werkzeuge zu öffnen.
• Die Stoppbehandlung im Wiedergabemenü wurde korrigiert: Die Taste Punkt verhält sich wie Stopp und beendet nur den aktuellen Titel, statt zusätzlich Player oder Episode zu verlassen.
• Für Medien aus „Zuletzt verwendete Dateien“, die aus einem lokalen Sonarpad-Cache stammen, wird im Wiedergabemenü nun ebenfalls die lokalisierte Speicheraktion angezeigt.
• Startet eine Transkription während laufender Audiowiedergabe, pausiert Sonarpad die Wiedergabe vorher automatisch.
• Ein erfolgreicher Wikipedia-Import zeigt den Artikeltext nun zuverlässig auf dem Bildschirm an.
• Eingebettete Podcast-Kapitel lokaler Mediendateien werden unterstützt. Wenn keine Kapitel aus Feed oder URL vorhanden sind, liest Sonarpad sie im Hintergrund aus der heruntergeladenen Datei; die Wiedergabe beginnt sofort und die Kapiteldaten werden nachgereicht.
• Auch heruntergeladene Podcast-Episoden, die als normale lokale Mediendateien geöffnet werden, erhalten eingebettete Kapitel und nicht nur Episoden, die aus dem Podcast-Fenster gestartet wurden.
• Die abschließende Verarbeitung von MP3-Hörbüchern mit SAPI4 und SAPI5 wurde korrigiert, sodass nach langen Exporten keine unvollständigen oder empfindlichen Dateien entstehen.
• Für sämtliche Hörbuchmodi gibt es eine ausdrückliche Fortschrittsanzeige der Abschlussphase. Nach der Erstellung wird die Finalisierung angesagt und sichtbar dargestellt.
• Einstellungen für Geschwindigkeit, Tonhöhe und Lautstärke werden nun für die erste und zweite Dialogstimme korrekt angewendet.
• Die Zeichencodierung japanischer TXT-Dateien wird besser erkannt. Für Fälle mit fehlerhaften Zeichen wurde ein sicherer Shift_JIS-/CP932-Ersatz ergänzt, ohne das bestehende Verhalten für UTF, diakritische Zeichen oder Chinesisch zu beeinträchtigen.
• Interne Sicherheitsüberarbeitung: Funktionen wurden, soweit möglich, in sichere Implementierungen umgewandelt und die Anzahl unsicherer Codezeilen deutlich verringert.

Version 0.6.7 – 2026-03-02

Verbesserungen
• „Alle ersetzen“ kann nun auch in großen Dateien mit sehr vielen Ersetzungen zuverlässig und schnell arbeiten.
• Die polnische Übersetzung wurde dank DJ Graco aktualisiert.
• Eine litauische Übersetzung wurde hinzugefügt.
• Eine chinesische Übersetzung wurde hinzugefügt.
• Häufige Betaversionen werden künftig im Releases-Bereich des Projekts veröffentlicht, damit Änderungen vor der nächsten stabilen Version getestet werden können.
• Strg+Punkt fügt das Auslassungszeichen (…) ein.
• Die Unterstützung von Podcast-Kapiteln wurde verbessert. Die Navigation funktioniert auch bei direkt oder gestreamt abgespielten Episoden, deren Kapitel nicht in der MP3-Datei eingebettet sind, indem verfügbare Feed-/URL-Metadaten verwendet werden. Strg+Alt+Bild Auf wechselt zum vorherigen, Strg+Alt+Bild Ab zum nächsten Kapitel.
• Sonarpad-Ausgabeordner wurden unter `Dokumente\Sonarpad` neu geordnet: `audiobooks`, `documents`, `recordings` und `media`. Dateien aus früheren Pfaden werden automatisch verschoben.
• Sehr große Textdateien, einschließlich Dateien mit 60 MB, öffnen sich flüssiger und lassen sich insbesondere mit Screenreadern besser zeilenweise navigieren.
• Die Handbücher aller Sprachen und die Lokalisierungsressourcen wurden aktualisiert, einschließlich Spendendateien und NSIS-Setup-Übersetzungen. Neu sind vereinfachtes Chinesisch und Litauisch im Installer sowie eine vervollständigte ukrainische Setup-Übersetzung.
• Ein globaler Netzwerkproxy für Online-Funktionen unterstützt HTTP/HTTPS und SOCKS5/SOCKS5H. Beim Speichern der Optionen wird er geprüft; ungültige Proxys werden gemeldet und automatisch entfernt.
• „Streaming-Audio wiedergeben...“ wurde dem Menü Werkzeuge hinzugefügt. Benutzer können eine URL zu YouTube oder einem direkten Medium einfügen, Ausgabeformat und Qualitäts-/Bitratenprofil einschließlich Originalqualität für MP3 und MP4 wählen und den Inhalt direkt im Sonarpad-Player abspielen.
• Die Systemtaste Wiedergabe/Pause an Headsets und Tastaturen steuert nun sowohl Medienwiedergabe als auch Pause/Fortsetzen der Textwiedergabe. Läuft beides, hat die Medienwiedergabe Vorrang.
• Datei > Zuletzt verwendete Dateien enthält „Liste leeren“, um den Verlauf schnell zu löschen.
• Die Bitratenauswahl in „Audio konvertieren“ und den Podcast-Aufnahmeeinstellungen wurde um 64 und 96 kbit/s sowie MP3 bis 320 kbit/s erweitert; Prüfung und Encoderbehandlung wurden angeglichen.
• Die Zeitoptionen zur Teilung von Hörbüchern reichen nun bis 60 Minuten.
• Bei der Hörbuchteilung nach Anzahl kann die gewünschte Zahl von 1 bis 100 manuell eingegeben werden.
• Ansicht > Schreibgeschützter Modus schützt den Editor vor unbeabsichtigten Änderungen, ohne Lesen und Navigation einzuschränken.
• Während Programmaktualisierungen zeigt eine barrierefreie Fortschrittsanzeige den Downloadfortschritt für Screenreader in Echtzeit an.
• Die Hauptfenster-Statusleiste zeigt ruhig Zeichen, Wörter sowie Zeile und Spalte an, beispielsweise „Zeichen (mit Leerzeichen): 11 | Wörter: 2 | Z. 1, Sp. 12“, ohne den NVDA-Fokus zu stören.
• Im Menü Ansicht kann der Zeilenumbruch direkt ein- und ausgeschaltet werden.
• Bearbeiten > Text enthält Einrücken und Ausrücken mit Strg+Umschalt+Punkt beziehungsweise Strg+Umschalt+Komma, da Tab bei eingeblendeten Stimmen im Editor für die Navigation im Stimmbereich reserviert ist.
• Datum und Uhrzeit in RSS-Artikeln und Podcast-Episoden werden lokalisiert und an die aktuelle Oberflächensprache angepasst.
• Das RSS-Kontextmenü kann den ausgewählten Artikel per E-Mail teilen.
• Unter Optionen > RSS und Podcast können Löschbestätigungen getrennt festgelegt werden: RSS für Quelle, Artikel, beides oder nichts; Podcasts für Podcast, Episode, beides oder nichts.
• Das Verhalten von Strg+C in RSS ist konfigurierbar: Titel, URL, Artikelinhalt oder alles gemeinsam kopieren.
• „Quelle hinzufügen“ akzeptiert direkte Feed-URLs oder Suchbegriffe und erzeugt aus Suchbegriffen automatisch Google-News-RSS. Eine getrennte Stichwortsuche ist nicht mehr nötig.
• Strg+A meldet nach der Auswahl den Abschluss für eine klarere Rückmeldung mit Screenreadern.
• Umschalt+F3 wurde für „Vorheriges suchen“ ergänzt; F3 bleibt „Nächstes suchen“.
• Rückmeldungen zu Ersetzungen verwenden korrekte Einzahl und Mehrzahl, etwa „1 Ersetzung vorgenommen“ und „2 Ersetzungen vorgenommen“.
• Im Wörterbuchfenster kann die Nachschlagesprache gewählt werden. Standard ist Automatisch anhand der Oberfläche; eine manuelle Sprache kann überschrieben werden.
• Die neue Registerkarte Tastenkürzel in den Optionen ermöglicht eigene Tastenkombinationen. Eine Konfliktprüfung warnt, wenn ein Kürzel bereits einer anderen Aktion zugeordnet ist.
• Erste Kommandozeilenoptionen wurden hinzugefügt: `-h`/`--help` zeigt die Verwendung und `--version` die Programmversion.
• Manuelle Werte für Geschwindigkeit und Tonhöhe verwenden nun verständlich eine Skala mit 100 als Normalwert.
• Die Auswahl von Microsoft-Stimmen unter Optionen > Stimme und im Stimmbereich des Editors erhielt ein lokalisiertes Sprachkombinationsfeld. Im Modus Nur mehrsprachige Stimmen bleibt eine gemeinsame Liste sichtbar und das Sprachfeld wird ausgeblendet.
• Unter Optionen > Stimme können Dialogstimmen vollständig per Tab konfiguriert werden: Engine, Edge-Sprachfilter, Stimme sowie beschriftete Werte für Geschwindigkeit, Tonhöhe und Lautstärke. Optional kann eine zweite Dialogstimme mit denselben Einstellungen für abwechselnde Dialoge eingerichtet werden. Regeln werden in einer `.ini`-Datei gespeichert und verändern den Dokumenttext nicht.
• Der Eintrag Rückgängig im Menü Bearbeiten zeigt nun die rückgängig zu machende Aktion an, etwa Texteingabe, Zeilen zitieren beziehungsweise Zitat entfernen oder Einfügen eines Sprach-Tags. Ohne verfügbare Aktion bleibt er deaktiviert.

Fehlerbehebungen
• RTF-Dateien werden nun als lesbarer Klartext dargestellt und nicht als Rohmarkup wie `{\rtf1...}`.
• Chinesische Textdateien in GB18030/GBK werden korrekt erkannt und ohne Zeichensalat geöffnet.
• M4B-Hörbücher erhielten bessere Kapitelmetadaten und Kapitelmarken; das Problem mit zu hoher Tonhöhe und Geschwindigkeit bei erzeugten M4B-Dateien wurde behoben.
• Im Hörbuch-Speicherdialog wurden fest eingetragene italienische Bitratenbeschriftungen entfernt und 64 kbit/s hinzugefügt.
• „Alle speichern“ mit Strg+Umschalt+S erkennt alle geänderten Dokumente zuverlässig, auch neue ungespeicherte Registerkarten, und speichert jede Datei oder öffnet bei Bedarf „Speichern unter“.
• Google-News-RSS-Artikel werden bei vorhandenen Datumsangaben absteigend nach Veröffentlichungsdatum angezeigt, die neuesten zuerst.
• Die Beschriftungszuordnung für NVDA im Wörterbuchfenster wurde korrigiert; Suchfeld und Sprachkombinationsfeld sagen nun die richtigen Beschriftungen an.
• Im Eigenschaftenfenster von RSS und Podcasts erreicht Tab beziehungsweise Umschalt+Tab die Schaltfläche OK, Eingabe aktiviert sie, Esc schließt sicher, und der Fokus kehrt korrekt zur RSS-/Podcast-Liste zurück.
• Strg+Z unterstützt bei RSS und Podcasts nun mehrstufiges Rückgängigmachen von Löschungen, sowohl Artikeln/Episoden als auch ganzen Quellen, statt nur der letzten Aktion.
• Rückmeldungen nach dem Entfernen von RSS-/Podcast-Inhalten nennen ausdrücklich, ob eine RSS-Quelle, ein RSS-Artikel oder eine Podcast-Episode entfernt wurde.
• Fokus und Ansagen nach Löschen oder Rückgängig wurden verbessert. RSS fokussiert bei Bedarf zuverlässig die erste Quelle und vermeidet wiederholte Screenreader-Ansagen bei verzögerter Neuauswahl.

Version 0.6.6 – 2026-02-13

Verbesserungen
• „Automatisch für Sprachausgabe formatieren“ wurde dem Menü Bearbeiten hinzugefügt. Es entfernt Markdown und Zitatzeichen und verbindet umgebrochene Zeilen für eine bessere Sprachausgabe.
• Beim Einfügen von Sprach-Tags werden ein- und mehrzeilige Markierungen nun korrekt verarbeitet.
• In den Audioeinstellungen kann ein Standardordner für Hörbücher gewählt werden; voreingestellt ist Dokumente\Sonarpad Audiobooks.
• Bei aktivierter Hörbuchteilung bietet der Speicherdialog eine standardmäßig aktivierte Option, die Teile in einem eigenen Unterordner abzulegen.
• Hörbücher mit Edge-, SAPI5- und SAPI4-Stimmen werden als Stereo-MP3 mit der vom Benutzer gewählten Bitrate gespeichert.
• 32-Bit-SAPI5-Stimmen werden über eine Bridge unterstützt und können dadurch auch in der 64-Bit-Anwendung verwendet werden.
• Sprachfunktionen wurden im eigenen Menü „Stimme und Audio“ zusammengefasst. „Audio konvertieren“ wurde ergänzt und unterstützt MP3, AAC, OGG, Opus, FLAC, WAV und AIFF.
• Einzelne RSS-Artikel und Podcast-Episoden können mit Entf oder über das Kontextmenü nach Bestätigung gelöscht werden, ohne die gesamte Quelle zu entfernen. Die letzte Löschung lässt sich rückgängig machen.
• RSS-Quellen können im RSS-Fenster nach OPML exportiert und später wieder importiert werden.
• „RSS nach Stichwort suchen“ erzeugt automatisch eine Google-News-RSS-URL und öffnet den Dialog zum Hinzufügen bereits ausgefüllt.
• Eine serbische Übersetzung wurde dank Mila Kuran hinzugefügt.
• Eine ukrainische Übersetzung wurde dank Ivan Shtefuriak hinzugefügt.
• Beim Öffnen mehrerer Mediendateien entsteht nun eine Wiedergabewarteschlange, statt die aktuelle Datei jeweils zu ersetzen.
• Variable Sprungtasten wurden hinzugefügt: Bei einem Grundwert von einer Minute springen Links/Rechts 60 Sekunden, Umschalt+Links/Rechts 20 Sekunden und Strg+Links/Rechts drei Minuten.
• Strg+Bild Auf und Strg+Bild Ab wechseln zum vorherigen beziehungsweise nächsten Titel.
• „Lautstärke zurücksetzen“ wurde hinzugefügt; Lautstärke, Geschwindigkeit und Tonhöhe befinden sich nun gemeinsam im Untermenü Zurücksetzen.
• Im Setup kann zwischen der Verknüpfung aller unterstützten Dateitypen und einer manuellen Erweiterungsauswahl gewählt werden. Das MSI bietet die Erweiterungen als einzelne Features an; standardmäßig sind alle aktiviert.
• Das neue Menü Fenster enthält „Geöffnete Dokumente...“ zum schnellen Wechsel zwischen Dateien.
• Ansicht > Schriftart verwendet nun ein schnelles Untermenü mit Arial, Calibri, Consolas, Segoe UI, Tahoma, Verdana, Times New Roman und Georgia und behält die aktuelle Textgröße bei.
• RSS- und Podcast-Ansagen verwenden zwei Statusarten: Quellen melden neue Elemente, einzelne Artikel beziehungsweise Episoden melden ungelesen oder ungehört. Dies kann in den Optionen deaktiviert werden.

Fehlerbehebungen
• EPUB-Text mit eingebetteten HTML-Kommentaren (`<!-- ... -->`) wird vollständig ausgewertet und nicht mehr teilweise übersprungen.
• Spanische Wiktionary-Suchen und der Wörterbuch-Cache wurden korrigiert; Einträge wie „agua“ laden wieder korrekt und alte „Wort nicht gefunden“-Cacheeinträge werden nicht wiederverwendet.
• RSS-Artikel einiger spanischer Quellen, etwa El Mundo, behalten Akzente und ñ beim Import korrekt bei.
• Die ANSI-Erkennung für mitteleuropäische Dateien wurde verbessert. Sonarpad unterscheidet UTF-8 und ANSI zuverlässiger und verwendet unter anderem Windows-1250, um beschädigte diakritische Zeichen zu vermeiden.
• RSS-Quellen mit URL-Abfrageparametern wie `rss.aspx?c=...` werden nach einem Neustart korrekt gespeichert und wiederhergestellt.
• Google-Drive-Verweisdateien (`.gdoc`, `.gsheet`, `.gslides`) werden bei Lesefehler „Incorrect function (os error 1)“ über die Windows-Shell geöffnet.
• Alte binäre Excel-2010-Dateien im XLS-Format werden erkannt und als Inhalt statt als Zeichensalat wie `ÐÏ_à¡±...` dargestellt.
• Rechtschreibfehler werden bei einer späteren Prüfung erneut angesagt; wird derselbe Fehler gelöscht und neu geschrieben, wird er ebenfalls wieder gemeldet.
• Zeilenbezogene Aktionen wie Strg+Q, Strg+Umschalt+Q, Sortieren, Umkehren, Duplikate entfernen und Zeilen verbinden schneiden bei einer mit Umschalt+Pfeil Ab markierten Einzelzeile keine Nachbarzeilen mehr ab.
• CR-getrennte mehrzeilige RichEdit-Markierungen werden normalisiert, sodass sämtliche ausgewählten Zeilen verarbeitet werden, ohne erste Zeichen abzuschneiden.
• Sichtbare Leerraumzeichen wie ␠, ␣, ␉, ␊, ␍ und ␤ werden für die Sprachausgabe normalisiert, um wiederholte Absätze mit mehrsprachigen Stimmen zu vermeiden.
• Edge-TTS verwendet eine gemeinsame Bereinigung: ungewöhnliche oder unsichtbare Leerzeichen werden normalisiert, lange Satzzeichenfolgen wie „...“, „!!!“ und „???“ verdichtet und Abschnitte nur aus Satzzeichen übersprungen.
• Die Zeitansage mit Strg+I für MP3- und Podcast-Streams wird auf die Titeldauer begrenzt; läuft die Position über das Ende hinaus, wird die Wiedergabe automatisch gestoppt.
• Das Setup enthält weitere Sprachen wie Tschechisch, Polnisch, Französisch und Serbisch. Das MSI bleibt zur Vermeidung von Verwechslungen ein einzelnes en-US-Paket.
• „Öffnen mit Sonarpad“ wird bei der Deinstallation auch in alten Registrierungsszenarien zuverlässig entfernt.
• Pause und Fortsetzen mit SAPI5 funktionieren zuverlässig; F4 pausiert korrekt und die Fortsetzung beginnt an der erwarteten Position statt am Anfang.
• Nach Pause und Sprung mit Links/Rechts setzt Leertaste die Medienwiedergabe an der aktuellen Position fort, statt zu stoppen oder von vorn zu beginnen.

Version 0.6.5 – 2026-02-07

Verbesserungen
• Die spanische Übersetzung wurde dank Arturo Fernandez Rivas verbessert.
• EPUB-Hörbücher können nach Kapiteln geteilt werden.
• RSS-Importe öffnen eine eigene temporäre, lokalisierte Registerkarte. „Speichern unter“ wandelt sie in ein normales Dokument um.
• Screenreader-Meldungen werden, sofern verfügbar, nun auch an JAWS gesendet.

Fehlerbehebungen
• F5 beginnt exakt an der Cursorposition. Zuvor konnte die Wiedergabe einige Zeilen zu früh starten, weil Cursoroffset und CRLF-/UTF-16-Positionen nicht übereinstimmten.
• Ein Darstellungsproblem wurde behoben, bei dem das Überschreiben einer Markierung vorherigen Text bis zur nächsten Bewegung scheinbar verschwinden ließ.
• Cover- oder reine Bildseiten in EPUB-Dateien erzeugen keine gesprochenen CSS-Angaben wie „padding“ und keine Titel „Sconosciuto“ mehr.
• Die zeitbasierte Teilung von EPUB-Hörbüchern mit Edge TTS scheitert nicht mehr an leeren oder übergroßen Abschnitten mit „Edge audio not sent“.
• RSS-Artikel decodieren HTML-Entitäten wie `&quot;`, `&amp;`, `&lt;` und `&gt;`.
• Speichern und Speichern unter schlagen bei nicht überschreibbaren Formaten wie EPUB den bestehenden Dateinamen statt der ersten Textzeile vor.
• Podcasts mit neuen Episoden werden wieder als ungehört angekündigt; die englische Bezeichnung wurde professioneller formuliert.

Version 0.6.4 – 2026-02-05

Verbesserungen
• Das Programm wurde in Sonarpad umbenannt, um Klang und Audio als Schwerpunkt hervorzuheben.
• Im Wiedergabemenü kann bei Medien mit mehreren Audiospuren, etwa MKV-Dateien in mehreren Sprachen, die Spur gewählt werden.
• Podcasts kennzeichnen ungehörte Episoden deutlich vor dem Namen.
• Sprachwechsel durch Tags im Text wurde hinzugefügt. Beispiele:
  - Microsoft-Stimmen (Edge): `<voice edge it-IT-IsabellaNeural>Hello</voice>`
  - SAPI5-Stimmen: `<voice sapi5 Microsoft Helena Desktop>Hello</voice>`
  - SAPI4-Stimmen: `<voice sapi4 #1>Hello</voice>`
  - Mit Geschwindigkeit, Tonhöhe und Lautstärke: `<voice edge it-IT-ElsaNeural speed=-20 pitch=-5 volume=-10>Hello</voice>`
• Die Podcast-Kategorien wurden erweitert.
• PDF-Lesen erhielt einen automatischen Ersatz über PDFium.
• Der Artikelparser wurde für Fälle verbessert, in denen Inhalte nicht vollständig gelesen wurden.
• Tonhöhe zurücksetzen wurde dem Wiedergabemenü hinzugefügt.
• Über das Kontextmenü kann aus dem ausgewählten Text ein Hörbuch erstellt werden.
• Hörbücher können nach Dauer geteilt werden; der Name der ersten Datei ist wählbar.
• Die Autorenbezeichnung in Artikeln ist lokalisiert, zum Beispiel „by“, „di“ oder „par“.
• Einrückung kann mit Tabs oder Leerzeichen und einer Breite festgelegt werden; Tab und Umschalt+Tab rücken ausgewählte Zeilen ein oder aus.
• Die Markdown-Bereinigung behandelt Sternchen-Listen korrekt, wenn Aufzählungszeichen nicht beibehalten werden sollen.
• Optional kann der alte Name „Novapad“ im Fenstertitel und in Startmenü-Verknüpfungen verwendet werden.

Fehlerbehebungen
• SAPI4-Hörbücher werden wieder wie erwartet erstellt.
• Ein Sprung über das Ende einer Mediendatei startet die Wiedergabe nicht mehr von vorn.
• „In Dateien suchen“ öffnet mit Eingabe die richtige Fundstelle; Esc kehrt zu den Ergebnissen zurück.
• Die Anordnung der Registerkarten Allgemein, Stimme, Editor und Audio wurde verbessert, damit keine Steuerelemente fehlen oder abgeschnitten werden.
• Ein Lesezeichenproblem nach Änderung der Wiedergabegeschwindigkeit wurde behoben.
• Podcast-Index-Kategorien werden korrekt angezeigt.
• Apostrophe unterbrechen die Wiedergabe nicht mehr; getrennte Dialogwiedergabe wurde entfernt und durch Sprach-Tags ersetzt.

Version 0.6.3 – 2026-01-30

Verbesserungen
• Die Mikrofonerkennung wurde verbessert.
• Alle unterstützten Formate können sofort wiedergegeben werden.

Fehlerbehebungen
• Ein Absturz im Fenster der Podcast-Kategorien wurde behoben.

Version 0.6.2 – 2026-01-30

Neue Funktionen
• Dateien können mit Umschalt+F5 ausgeführt werden. In den Optionen lässt sich ein Interpreter wie Python auswählen oder auf dem Computer suchen; HTML-Dateien werden im Browser geöffnet.
• Google-Docs-Verweisdateien `.gdoc`, `.gsheet` und `.gslides` öffnen automatisch im Standardbrowser.
• Das Hörbuchformat M4B (Apple/AAC) wird unterstützt.
• Im Kontextmenü der Podcast-Suchergebnisse zeigt „Episoden anzeigen“ Episoden an und spielt sie ab, ohne den Podcast zu abonnieren.
• „Gehe zu Zeile“ im Menü Bearbeiten beziehungsweise Strg+J springt schnell zu einer Zeilennummer.
• RSS-Quellen und Podcasts können über das Kontextmenü alphabetisch oder nach Datum sortiert werden.
• Vietnamesische Standard-RSS-Quellen wurden hinzugefügt.
• Im Aufnahmedialog kann das Mikrofon vor Beginn anhand des Pegels getestet werden.
• Das Kontextmenü kann die Beschreibung einer Podcast-Episode anzeigen.
• FFmpeg erweitert die unterstützten Audio- und Videoformate um MKV, AVI, MOV, M4V, WebM, MPG, TS, WMV, FLV, VOB, 3GP, FLAC, OGG, WMA und AIFF.
• Synchronisierte Untertitel in SRT, VTT, ASS, SUB, SBV, LRC und SMI können mit NVDA oder der gewählten Stimme gelesen werden. Sonarpad sucht eine Untertiteldatei mit demselben Namen; für abweichende Namen stehen Importieren und Entfernen im Wiedergabemenü zur Verfügung.
• Alle neuen Audio- und Videoformate erhalten Dateizuordnungen für „Öffnen mit Sonarpad“.
• Die Tonhöhe kann für jede Datei angepasst werden.
• Unter Allgemein können anonyme Fehlerberichte aktiviert oder deaktiviert werden. Im Hilfemenü lässt sich ein Diagnose-ZIP erstellen.
• Für Dialoge kann bei Live-Wiedergabe und Hörbucherstellung eine andere Stimme verwendet werden.
• Podcasts können nach Kategorien wie Wirtschaft, Kunst oder Sport durchsucht werden.

Verbesserungen
• Eine aus Explorer geöffnete Audio- oder Videodatei zeigt direkt den Player statt des Texteditors.
• Bei nicht zugänglichen PDF-Dateien startet OCR automatisch ohne Rückfrage.
• Das barrierefreie Terminal merkt sich für NVDA die zuletzt gelesene Zeile.
• Die Hörbucherstellung mit SAPI4 läuft vollständig parallel und nahezu sofort; die Zahl gleichzeitiger Prozesse wird abgefragt.
• SAPI4 wandelt Abschnitte bereits während der Synthese parallel von WAV nach MP3 um und beseitigt damit den bisherigen Engpass.
• Fehlerbehandlung und automatisches Löschen temporärer SAPI4-Dateien wurden verbessert.
• Im Suchdialog heißt „Regex“ nun verständlicher „Regulärer Ausdruck“; fehlende Übersetzungen der Suchoptionen wurden ergänzt.
• M4B-Ausgabe wurde verbessert. Teilung nach Teilen oder Markierungen erzeugt eine einzelne M4B-Datei mit Kapitelmetadaten, Titel und Autor.
• Lesezeichen und Zeitansage im Player sind auch bei einer Geschwindigkeit ungleich 1,0 genauer.
• Strg+Tab und Strg+Umschalt+Tab navigieren wieder zwischen den Optionsregisterkarten.
• Das Wiedergabemenü kann die Geschwindigkeit sofort auf Normal (1,0x) zurücksetzen.
• Alle Abhängigkeiten wurden für Leistung und Stabilität aktualisiert.
• FFmpeg wird über dynamische DLLs eingebunden, ohne den Programmstart zu blockieren.
• Podcast-Downloadfilter berücksichtigen die neuen Audio- und Videoformate.
• Strg+S speichert keine Audio- oder Videodateien und verhindert dadurch Beschädigungen.
• Der Import von YouTube-Transkripten ist robuster.
• Die Teilung von Hörbuchtexten wurde stabiler, sodass kein Text verloren geht.
• Der Installer ist vollständig mehrsprachig und unterstützt Italienisch, Englisch, Spanisch, Portugiesisch, Schwedisch und Vietnamesisch anhand der Systemsprache. Andernfalls wird Englisch verwendet.
• Eingabe auf einer Podcast-Kategorie bestätigt die Auswahl wie die Schaltfläche OK.
• Die Erkennung hängender Prozesse vermeidet Fehlalarme bei geöffneten modalen Dialogen wie Fehlermeldungen oder „Text nicht gefunden“.

Korrekturen
• Das Änderungsprotokoll öffnet sich wieder beim Start.
• Die OCR-Abfrage für aus Explorer geöffnete unzugängliche PDF-Dateien wurde korrigiert.
• Ein Startfehler, der sofort Fokusverlust oder Fensterschließung verursachen konnte, wurde behoben.
• Die Suche mit regulären Ausdrücken findet wieder Text; „Am Ende von vorn beginnen“ und „Punkt entspricht Zeilenumbruch“ funktionieren mit Windows-Zeilenenden korrekt.

Lokalisierung
• Eine polnische Übersetzung wurde hinzugefügt.
• Eine französische Übersetzung wurde hinzugefügt.
• Eine tschechische Übersetzung wurde dank Radek Žalud und Jiri Holzinger hinzugefügt.

Version 0.6.1 – 2026-01-20

Korrekturen
• Das Einblenden der Stimmen im Editor beendet die Podcast-Wiedergabe nicht mehr.
• Podcast-URLs werden beim Hinzufügen nicht mehr abgeschnitten.
• Normale URLs können wieder als RSS-Quelle hinzugefügt werden.
• Die Wikipedia-Sprachoption erscheint nicht mehr mehrfach auf verschiedenen Optionsregisterkarten.
• Debugdateien werden in Release-Versionen nicht mehr irrtümlich erzeugt.

Verbesserungen
• Microsoft-Stimmen verwenden eine eigene Wiedergabemethode mit einem angepassten User-Agent.
• MP4-Dateien werden unterstützt.

Version 0.6.0 – 2026-01-20

Neue Funktionen
• Eine Rechtschreibprüfung wurde hinzugefügt. Über das Kontextmenü lässt sich das aktuelle Wort prüfen; bei Fehlern werden Korrekturvorschläge angezeigt.
• Podcasts können über OPML-Dateien importiert und exportiert werden.
• Zusätzlich zu iTunes wird die Suche über Podcast Index unterstützt. Benutzer können einen kostenlosen API-Schlüssel und ein Geheimnis eintragen, die nur mit einer E-Mail-Adresse erstellt werden.
• SAPI4-Stimmen werden für direkte Wiedergabe und Hörbucherstellung unterstützt.
• Nicht zugängliche PDF-Dateien erhalten einen automatischen OCR-Ersatz, wenn kein Text extrahiert werden kann.
• Wiktionary wurde als Wörterbuch hinzugefügt. Die Anwendungstaste zeigt Definitionen und, falls vorhanden, Synonyme und Übersetzungen in andere Sprachen.
• Wikipedia-Artikel können gesucht, aus einer Ergebnisliste gewählt und direkt in den Editor importiert werden.
• Umschalt+Eingabe öffnet im RSS-Modul einen Artikel direkt auf der ursprünglichen Website.

Verbesserungen
• Die gewählte Mikrofonquelle wird stets berücksichtigt.
• Eingabe auf einer Podcast-Episode meldet sofort „Wird geladen“ über NVDA.
• Eingabe in Podcast-Suchergebnissen abonniert den ausgewählten Podcast.
• Beschriftungen der Kürzel Strg+Umschalt+O und Podcast Strg+Umschalt+P wurden korrigiert und verbessert.
• Wiedergabegeschwindigkeit und Lautstärke werden gespeichert und gelten über alle Audiodateien hinweg.
• Podcast-Episoden verwenden einen eigenen Cacheordner. „Podcast behalten“ im Wiedergabemenü schützt eine Episode; der Cache wird automatisch bereinigt, wenn die unter Optionen > Audio festgelegte Größe überschritten wird.
• RSS-Artikel werden mit libcurl-Imitation von Chrome- und iPhone-Profilen geladen, wodurch ungefähr 99 Prozent der Websites unterstützt werden.
• RSS-Artikel zeigen einen klaren gelesen-/ungelesen-Status.
• „Alle ersetzen“ meldet die Zahl der vorgenommenen Ersetzungen.
• Bei der Navigation durch die Podcast-Bibliothek mit Tab steht eine Schaltfläche zum Löschen des Podcasts zur Verfügung.

Korrekturen
• Der überflüssige Eintrag „Ausstehende Aktualisierung“ wurde aus dem Hilfemenü entfernt, da Aktualisierungen bereits automatisch verarbeitet werden.
• Strg+S kann eine geöffnete MP3-Datei nicht mehr speichern und beschädigen.
• Die fehlerhafte Anzeige „(B)… Strg+Umschalt+B“ für Stapel-Hörbücher wurde korrigiert.
• Bei aktivierten typografischen Anführungszeichen werden normale Anführungszeichen nun korrekt ersetzt.
• „Gehe zu Lesezeichen“ setzt die Wiedergabegeschwindigkeit nicht mehr auf 1,0 zurück.
• Bereits heruntergeladene Podcast-Episoden werden aus dem Cache verwendet und nicht erneut geladen.

Tastenkürzel
• F1 öffnet das Handbuch.
• F2 sucht nach Aktualisierungen.
• F7 und F8 wechseln zum vorherigen beziehungsweise nächsten Rechtschreibfehler.
• F9 und F10 wechseln schnell zwischen bevorzugten Stimmen.

Verbesserungen für Entwickler
• Fehler werden nicht mehr stillschweigend verworfen. Sämtliche Muster `let _ =` wurden entfernt; Fehler werden weitergegeben, protokolliert oder ausdrücklich mit Ersatzverhalten behandelt.
• Das Projekt kompiliert bei Warnungen nicht mehr. `cargo check` und `cargo clippy` müssen ohne Warnungen bestehen; Lints wurden verschärft und `allow` soweit möglich entfernt.
• Eigene Implementierungen wie strlen-/wcslen-Hilfen wurden entfernt. Längen von Zeichenketten und UTF-16-Puffern werden aus Rust-eigenen Daten abgeleitet, statt Speicher zu durchsuchen.
• DLL-Behandlung wurde bereinigt und um `libloading` vereinheitlicht; eigene Loaderlogik und PE-Auswertung wurden entfernt.
• Selbst geschriebene Byteparser wurden entfernt. Bytewerte werden mit `from_le_bytes` und `from_be_bytes` aus geprüften Slices gelesen.
• Diese Änderungen verringern unnötigen unsicheren Code, vermeiden mögliches undefiniertes Verhalten und machen die Codebasis idiomatischer, robuster und wartbarer.

Version 0.5.9 – 2026-01-13

Neue Funktionen
• RSS-Quellen können über das Kontextmenü nach oben, unten oder an eine bestimmte Position verschoben werden; ungültige Positionen werden geprüft.
• Das Artikel-Kontextmenü öffnet die ursprüngliche Website und teilt über WhatsApp, Facebook und X.
• Esc kehrt von importierten Artikeln zur RSS-Liste zurück.
• Der Podcast-Modus unterstützt Suche, Abonnement und Wiedergabe. Abonnements können sortiert werden; Esc stoppt die Wiedergabe und kehrt zur Liste zurück; Eingabe auf einer Episode startet sie.
• Die Wiedergabegeschwindigkeit von Podcasts und MP3-Dateien kann geändert werden.
• Strg+T springt zu einer bestimmten Zeit.
• Nach dem Lautstärkekombinationsfeld wurde eine Schaltfläche zur Stimmvorschau hinzugefügt.
• Suchen und Ersetzen unterstützt reguläre Ausdrücke nach Art von Notepad++.
• RSS-Quellen können aus OPML- und TXT-Dateien importiert werden.
• „Öffnen mit Sonarpad“ kann in Explorer aktiviert werden, auch für portable Versionen.

Verbesserungen
• Auswahl von Geschwindigkeit, Tonhöhe und Lautstärke respektiert die Höchstwerte der jeweiligen TTS-Engine.
• RSS lädt alle Artikel, ohne den NVDA-Fokus während Aktualisierungen zu verschieben.
• Die Audiowiedergabe erhielt ein eigenes Menü, Strg+I zur Zeitansage und Lautstärke bis 300 Prozent.
• Fehlende Tastenkürzel für einige Funktionen wurden ergänzt.
• Das Menü Bearbeiten enthält ein Untermenü zur Textbereinigung.
• Die Optionen wurden in Registerkarten gegliedert und können mit Strg+Tab sowie Strg+Umschalt+Tab navigiert werden.
• Der RSS-Reader lädt den vollständigen Artikelinhalt entsprechend der Browseransicht.

Korrekturen
• Die Markdown-Bereinigung entfernt keine Zahlen mehr am Zeilenanfang.
• AltGr+Z löst nicht mehr Rückgängig aus.
• Das Abbrechen einer Hörbuchaufnahme beendet den Vorgang schnell.

Lokalisierung
• Eine vietnamesische Übersetzung wurde dank Anh Đức Nguyễn hinzugefügt.

Version 0.5.8 – 2026-01-10

Neue Funktionen
• Bei Podcast-Aufnahmen können Mikrofon- und Systemlautstärke getrennt eingestellt werden.
• Artikel können aus Websites oder RSS-Quellen importiert werden; für jede Sprache sind wichtige Standardquellen enthalten.
• Alle Lesezeichen der aktuellen Datei können entfernt werden.
• Doppelte Zeilen und direkt aufeinanderfolgende doppelte Zeilen können entfernt werden.
• Alle Registerkarten oder Fenster außer dem aktuellen können geschlossen werden.
• Der Eintrag Spenden wurde in allen Sprachen dem Hilfemenü hinzugefügt.

Verbesserungen
• Das barrierefreie Terminal wurde verbessert, um Abstürze zu vermeiden.
• Zugriffstasten und Tastenkürzel wurden programmweit verbessert und korrigiert.
• Das Schließen des Audiowiedergabefensters beendet nun die Wiedergabe.
• Wichtige Aktionen wie Duplikate entfernen, Trennstriche am Zeilenende entfernen und alle Lesezeichen löschen verlangen eine Bestätigung. Ist die Aktion nicht anwendbar, erscheint kein Dialog.
• RSS-Quellen können in der Bibliothek mit Entf gelöscht werden.
• Das RSS-Fenster besitzt ein Kontextmenü zum Bearbeiten oder Löschen von Quellen.
• Die Einstellung zum Verschieben der Konfiguration in den aktuellen Ordner wurde entfernt. Liegt die EXE in einem Ordner „sonarpad portable“ oder auf einem Wechseldatenträger, wird `config` neben der EXE verwendet; andernfalls `%APPDATA%\Sonarpad`. Ist der bevorzugte Ordner nicht beschreibbar, wird ebenfalls der lokale `config`-Ordner verwendet.

Version 0.5.7 – 2026-01-05

Neue Funktionen
• „Stapel-Hörbücher“ konvertiert mehrere Dateien oder Ordner in einem Arbeitsgang.
• Markdown-Dateien (`.md`) werden unterstützt.
• Beim Öffnen von Textdateien kann die Zeichencodierung gewählt werden.
• Das barrierefreie Terminal kann neue Zeilen über NVDA ansagen.

Verbesserungen
• Hörbuchaufnahmen werden bei MP3-Auswahl direkt als MP3 gespeichert.
• Die Position des Sternchens für ungespeicherte Änderungen im Fenstertitel kann gewählt werden.
• Das Aktualisierungssystem ist in unterschiedlichen Situationen robuster.
• „Trennstriche entfernen“ im Menü Bearbeiten korrigiert OCR-Zeilenenden.

Version 0.5.6 – 2026-01-04

Korrekturen
• „In Dateien suchen“ öffnet mit Eingabe die Datei genau an der ausgewählten Fundstelle.

Verbesserungen
• PPT- und PPTX-Dateien können als Text geöffnet werden.
• Beim Öffnen nicht textbasierter Formate wird zum Schutz vor Formatbeschädigung als TXT gespeichert: PDF, DOC, DOCX, EPUB, HTML, PPT und PPTX.
• Podcast-Aufnahmen von Mikrofon und Systemaudio wurden dem Menü Datei hinzugefügt und können mit Strg+Umschalt+R gestartet werden.

Version 0.5.5 – 2026-01-03

Neue Funktionen
• Ein barrierefreies, für große Ausgaben und Screenreader optimiertes Terminal wurde mit Strg+Umschalt+P hinzugefügt.
• Benutzereinstellungen können für den portablen Betrieb im aktuellen Ordner gespeichert werden.

Korrekturen
• Vorschauen in „In Dateien suchen“ bleiben korrekt an der Fundstelle ausgerichtet.

Version 0.5.4 – 2026-01-03

Verbesserungen
• „Leerraum normalisieren“ mit Strg+Umschalt+Eingabe wurde korrigiert.
• HTML- und HTM-Dateien können als Text geöffnet werden.

Version 0.5.3 – 2026-01-02

Neue Funktionen
• „In Dateien suchen“ wurde hinzugefügt.
• Neue Textwerkzeuge: Leerraum normalisieren, harter Zeilenumbruch und Markdown entfernen.
• Textstatistik ist mit Alt+Y verfügbar.
• Neue Listenbefehle im Menü Bearbeiten:
• Elemente sortieren (Alt+Umschalt+O)
• Nur eindeutige Elemente behalten (Alt+Umschalt+K)
• Reihenfolge umkehren (Alt+Umschalt+Z)
• Zeilen zitieren beziehungsweise Zitat entfernen (Strg+Q / Strg+Umschalt+Q).

Lokalisierung
• Eine spanische Lokalisierung wurde hinzugefügt.
• Eine portugiesische Lokalisierung wurde hinzugefügt.

Verbesserungen
• Bei geöffneten EPUB-Dateien wechselt Speichern automatisch zu Speichern unter und exportiert den Inhalt als TXT, damit das EPUB nicht beschädigt wird.

Version 0.5.2 – 2026-01-01
• Ein Änderungsprotokoll wurde hinzugefügt.
• Während der Installation können „Öffnen mit Sonarpad“ und Dateizuordnungen für unterstützte Formate eingerichtet werden.
• Meldungen, Fehlerdialoge und Hörbuchexport wurden besser lokalisiert.
• „Hörbuch anhand des Textes teilen“ erhielt eine Teileauswahl und die Option „Markierung muss am Zeilenanfang stehen“.
• YouTube-Transkripte können mit Sprachauswahl, optionalen Zeitstempeln und verbesserter Fokusbehandlung importiert werden.

Version 0.5.1 – 2025-12-31
• Automatische Aktualisierungen mit Bestätigung, besserer Fehlerbehandlung und Benachrichtigungen.
• Verbesserungen beim Hörbuchexport: textbasierte Teilung, SAPI5/Media Foundation und erweiterte Einstellungen.
• Verbesserungen der Sprachausgabe: Pause/Fortsetzen, Ersetzungswörterbuch und Favoriten.
• Menü Ansicht sowie Bereiche für Stimmen und Favoriten, Textfarbe und Textgröße.
• Standardsprache aus dem Systemgebietsschema und verbesserte Lokalisierung.
• CI und Windows-Paketierung mit Artefakten, MSI/NSIS und Cache.

Version 0.5.0 – 2025-12-27
• Modularer Umbau von Editor, Dateiverwaltung, Menü und Suche.
• Windows-Build- und Paketierungsablauf sowie Aktualisierungen von README und Lizenz.
• Tab-Navigation im Hilfefenster korrigiert.

Version 0.5 – 2025-12-27
• Vorläufige Erhöhung der Versionsnummer.

Version 0.1.0 – 2025-12-25
• Erste Veröffentlichung mit Projektstruktur und README.

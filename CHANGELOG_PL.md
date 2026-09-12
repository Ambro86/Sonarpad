# Dziennik zmian

Wersja 0.9.8 – 2026-09-12

Audiodeskrypcja AI
1. Dodano ostrożny mechanizm odzyskiwania eksportu wielokanałowego bez zmiany pipeline dla plików, które już działają. Sonarpad nadal najpierw używa oryginalnego układu kanałów i tylko w razie potrzeby uzupełnia niepełną ostatnią ramkę PCM. Jeśli mimo to pośredni wielokanałowy WAV nie może zostać zakończony, ponieważ liczba próbek nie jest wielokrotnością liczby kanałów, Sonarpad automatycznie ponawia tylko ten eksport w stereo. Poprawnie działające eksporty mono, stereo i wielokanałowe pozostają bez zmian.

Ciemny motyw i dostępność
1. Naprawiono problem, przez który ciemny motyw zakłócał natywne okna dialogowe Windows i komunikaty potwierdzenia. Sonarpad nie stosuje już własnego subclassingu ciemnego motywu do systemowych drzew dialogów `#32770`, dzięki czemu okna Otwórz/Zapisz, eksport diagnostyki i MessageBox pozostają obsługiwane przez Windows i prawidłowo otrzymują fokus klawiatury/NVDA. Naprawiono również czas życia domyślnego rozszerzenia ZIP w oknie zapisu diagnostyki.

Wersja 0.9.7 – 2026-09-11

Audiodeskrypcja AI
1. Dodano opcjonalną możliwość tworzenia audiodeskrypcji w oryginalnym pliku wideo. Po jej włączeniu Sonarpad preferuje teraz MP4: kopiuje oryginalny strumień wideo bez ponownego kodowania i dodaje zmiksowaną ścieżkę audiodeskrypcji. Jeśli podczas tworzenia MP4 FFmpeg zgłosi niezgodność kontenera lub zapisu pakietów, Sonarpad automatycznie ponawia ten sam eksport jako MKV, nadal bez ponownego kodowania wideo. MKV można też wybrać bezpośrednio. W tym trybie rozszerzone pauzy są ignorowane, aby zachować synchronizację dźwięku i obrazu.

2. Ulepszono ponowną analizę segmentów w zapisanych projektach audiodeskrypcji. Okno korzysta teraz z dostępu do AI skonfigurowanego wcześniej w głównym oknie Tworzenie audiodeskrypcji, więc nie powiela klucza API, salda Sonarpad AI ani wyboru modelu. Ponownie analizowany segment zachowuje teraz całą grupę opisów projektu należących do tego fragmentu analizy zamiast tylko pierwszego lub najbliższego opisu; wszystkie opisy w grupie są oznaczane jako ponownie przeanalizowane, a „Zastosuj segment i ponownie wyeksportuj MP3” stosuje całą grupę jednocześnie. Podglądy rozszerzonych pauz są teraz syntetyzowane bezpośrednio zamiast wyszukiwania pozycji w wyeksportowanym MP3, co zapobiega rozpoczynaniu podglądu od dźwięku filmu lub ucinaniu opisu.

3. Rozszerzono ostrożny mechanizm awaryjny multimediów Gemini bez zmiany już działającej ścieżki. HTTP 400 `INVALID_ARGUMENT` nadal uruchamia mniejsze fragmenty MKV (około 15 MB), a następnie zgodne fragmenty MP4. Jeśli Gemini przyjmie wysłany fragment, ale jego przetwarzanie zakończy się stanem `FAILED`, Sonarpad ponownie wysyła dokładnie ten sam fragment jeden raz i dopiero potem przechodzi do MP4; ponowienia dla serwerowego Code 13 są ograniczone do trzech, aby uniknąć nieskończonego oczekiwania. Błędy sieci, limitu, uwierzytelniania i syntezy nie uruchamiają tej ścieżki.

4. Poprawiono zapisywanie danych dostępu AI podczas przełączania trybu. Osobisty klucz API Gemini i kod dostępu Sonarpad AI są teraz zapisywane niezależnie: przełączanie między osobistym kluczem API a Sonarpad AI nie usuwa już nieaktywnego poświadczenia. Klucz Gemini jest także zapisywany po opuszczeniu jego pola.

5. Dodano prawdziwy przycisk Anuluj dla „Ponownie przeanalizuj segment” oraz „Zastosuj segment i ponownie wyeksportuj MP3”. Pasek postępu pozostaje aktywny, a Anuluj przerywa teraz przygotowanie segmentu przez FFmpeg, proces roboczy AI, weryfikację TTS i końcowy eksport, gdy tylko bieżący etap może zostać bezpiecznie zatrzymany. Zastosowanie ponownie przeanalizowanego segmentu jest teraz transakcyjne: istniejący projekt i plik wynikowy są zastępowane dopiero po pomyślnym ponownym eksporcie; po anulowaniu poprzednie pliki pozostają bez zmian, a propozycja ponownej analizy jest nadal dostępna do kolejnej próby.

6. Naprawiono ponowną analizę segmentów dla opisów znajdujących się poza pierwszym fragmentem Gemini. Izolowany fragment jest teraz wysyłany do istniejącego workera z lokalną osią czasu zaczynającą się od 0, a następnie mapowany z powrotem na bezwzględną pozycję w projekcie, co zapobiega błędowi `Invalid prepared chunk timeline at chunk 1` bez zmiany standardowego potoku audiodeskrypcji.

7. Ponowna analiza segmentów w zapisanych projektach korzysta teraz z tych samych kontroli bezpieczeństwa co pełne tworzenie audiodeskrypcji. Wybrany fragment przechodzi tę samą ekstrakcję dźwięku mono 16 kHz i analizę dialogów/ciszy Pyannote, te same reguły czasowe i mechanizmy awaryjne Gemini, tę samą rzeczywistą syntezę TTS głosem projektu oraz ten sam końcowy harmonogram bezpiecznego umieszczania i wydłużonych pauz. Przy zastosowaniu segmentu Sonarpad zachowuje nowe, zweryfikowane czasy bezwzględne i odświeża ochronę Pyannote dla tego fragmentu; usunięto wcześniejsze specjalne obejście podglądu dla zbyt długiego tekstu.

8. Poprawiono strukturalną wymianę segmentu po pełnej ponownej analizie. Sonarpad nie wymaga już, aby nowo zweryfikowany segment zawierał dokładnie tyle samo opisów co zapisany projekt: stare opisy danego fragmentu są zastępowane całym bezpiecznym wynikiem pełnego potoku, więc segment z 10 opisami może poprawnie zmienić się w 9 (lub 11). Edytor projektu od razu pokazuje nową liczbę i czasy do odsłuchu, a zapisany projekt i plik multimedialny pozostają bez zmian do pomyślnego zakończenia zastosowania segmentu i ponownego eksportu MP3.

9. Przebudowano ponowną analizę segmentu tak, aby wybrany fizyczny fragment był traktowany jak prawdziwy, samodzielny mini-film. Sonarpad przekazuje teraz ten mini-film bezpośrednio do dokładnie tej samej funkcji `create_audio_description`, której używa zwykłe pełne tworzenie, dzięki czemu obraz i ścieżka dźwiękowa mają tę samą lokalną oś czasu, a standardowe kontrole Pyannote, Gemini, rzeczywistego TTS, harmonogramowania i bezpieczeństwa działają bez osobnej implementacji ponownej analizy. Dopiero po zakończeniu normalnego procesu wynikowe czasy lokalne są przesuwane z powrotem na pierwotną pozycję w filmie.

Edycja tekstu
1. Ulepszono funkcję „Połącz zawinięte wiersze” w Edycja > Tekst. Bez zaznaczenia przetwarza cały dokument, a z zaznaczeniem tylko wybrane wiersze. Teraz odtwarza pełne akapity, łącząc kolejne wiersze także wtedy, gdy zdanie kończy się znakiem interpunkcyjnym; puste wiersze pozostają separatorami akapitów, a listy numerowane i punktowane są zachowywane.

Wersja 0.9.6 – 2026-09-09

1. Naprawiono zawieszanie się niektórych głosów SAPI4 przy włączonym śledzeniu kursora. Mostek czeka teraz na rzeczywiste zakończenie syntezy, zachowując śledzenie kursora.

2. Poprawiono synchronizację mikrofonu i dźwięku systemowego oraz usunięto trzaski powodowane zaokrąglaniem znaczników czasu podczas nagrywania podcastów. Poprawki dotyczą także oddzielnego zapisywania źródeł.

3. Synteza audiodeskrypcji SAPI5 działa teraz w osobnych procesach. Po błędzie jest ponawiana z mniejszą równoległością, zachowuje udane opisy i zapamiętuje działający limit dla głosu.

Wersja 0.9.5 – 2026-09-07

Audiodeskrypcja z AI
1. Zwiększono odporność na sytuację, gdy Gemini tymczasowo zwraca `MALFORMED_RESPONSE` bez użytecznej treści. Sonarpad ponawia teraz dokładnie ten sam fragment do trzech razy, z krótką przerwą między próbami, zamiast natychmiast przerywać całą audiodeskrypcję. Zwykłe odpowiedzi Gemini, blokady bezpieczeństwa, błędy limitu tokenów oraz dotychczasowe działanie punktów kontrolnych pozostają bez zmian.

2. Naprawiono kolejny rzadki przypadek błędu `invalid Gemini chunk timeline` w niektórych filmach, gdy fragmenty przygotowane przez FFmpeg zgłaszają niewielkie skumulowane odchylenie czasu trwania. Sonarpad pozostawia dotychczasową ścieżkę bez zmian, gdy oś czasu jest prawidłowa, i stosuje ostrożne wyrównanie tylko wtedy, gdy normalna oś czasu zakończyłaby się błędem, a łączna różnica jest mała; duże lub podejrzane rozbieżności nadal powodują błąd.

3. Dodano opcjonalną usługę Sonarpad AI do audiodeskrypcji z AI. Użytkownicy mogą nadal korzystać z własnego klucza API Gemini albo wybrać usługę Sonarpad i wpisać osobisty kod Sonarpad. Kod jest chroniony przez Windows DPAPI. W trybie usługi przygotowane fragmenty wideo są wysyłane bezpośrednio z komputera użytkownika do Google przez tymczasowy adres URL przesyłania Google; sonarpad.com nie otrzymuje ani nie przechowuje danych wideo. Backend jedynie autoryzuje dostęp, wymusza model Gemini/poziom Standard i limity użycia, rozlicza wykorzystanie oraz zwraca odpowiedź Gemini.

4. Ulepszono edycję zapisanych projektów audiodeskrypcji. Można teraz zmieniać kilka opisów po kolei bez konieczności natychmiastowego stosowania każdej zmiany. Zmiany pozostają w pamięci; po naciśnięciu „Zastosuj tekst” Sonarpad sprawdza każdy zmodyfikowany opis względem zapisanego zakresu czasu, a następnie zapisuje wszystkie prawidłowe zmiany razem. Jeśli opis nie mieści się w swoim zakresie, fokus wraca do niego bez utraty pozostałych oczekujących zmian.

5. Dodano „Dostosuj głos” bezpośrednio przed „Utwórz audiodeskrypcję”. Nowe okno pozwala wybrać silnik, głos, szybkość i głośność oraz zawiera „Testuj głos”. Po naciśnięciu OK fokus wraca dokładnie do „Dostosuj głos”, a wybrane wartości są używane podczas tworzenia audiodeskrypcji. Jeśli okno nie zostanie użyte, synteza działa dokładnie tak jak wcześniej.


6. Rozszerzono sterowanie głosem w zapisanych projektach audiodeskrypcji. W oknie edycji projektu oprócz silnika i głosu dostępne są teraz także szybkość, głośność oraz „Testuj głos”. Po zastosowaniu ustawień głosu Sonarpad ponownie sprawdza wszystkie istniejące opisy z nowymi parametrami syntezy i przebudowuje plik MP3 tylko wtedy, gdy wszystkie nadal mieszczą się w zapisanych przedziałach czasowych. Przedziały bez dialogów, czasy Visual Evidence, opisy Gemini i analiza czasowa projektu są ponownie wykorzystywane bez ponownego uruchamiania analizy AI.

Wersja 0.9.4 – 2026-09-04

Audiodeskrypcja z AI
1. Naprawiono problem z filmami Matroska/WebM zawierającymi nietypowo wysoki wewnętrzny znacznik czasu rozpoczęcia, który mógł zatrzymać generowanie audiodeskrypcji z błędem „invalid Gemini chunk timeline”. Sonarpad normalizuje teraz czas trwania pliku źródłowego i fragmentów Gemini tylko po wykryciu tego nietypowego układu znaczników czasu, pozostawiając zwykłe filmy oraz dotychczas działające zachowanie audiodeskrypcji bez zmian.
2. Ulepszono ducking audiodeskrypcji, aby miks brzmiał bardziej naturalnie. Oryginalny dźwięk jest teraz płynnie ściszany jeszcze przed rozpoczęciem głosu i stopniowo wraca do normalnego poziomu po zakończeniu opisu; przy opisach znajdujących się bardzo blisko siebie poziom tła pozostaje stabilny, bez ciągłego ściszania i podgłaśniania.
3. Naprawiono problem, który mógł powodować błąd końcowego eksportu audiodeskrypcji w przypadku niektórych plików AVI i innych źródeł z dźwiękiem 5.1/wielokanałowym, gdy dekodowanie kończyło się niepełną przeplataną ramką audio. Sonarpad bezpiecznie uzupełnia teraz wyłącznie tę ostatnią częściową ramkę dokładnie taką liczbą próbek ciszy, jaka jest konieczna; kompletne ramki oraz zwykłe pliki mono, stereo i wielokanałowe pozostają bez zmian.


Wersja 0.9.3 – 2026-09-03

Głosy SAPI5
1. Naprawiono problem, przez który niektóre lokalne głosy SAPI5 mogły nie mówić przy włączonym przesuwaniu kursora, podczas czytania wieloma głosami lub tworzenia audiobooków/MP3. Sonarpad używa teraz ścieżki syntezy SAPI5 do pliku, która działa niezawodnie w Windows i nadal pozwala anulować syntezę, a zwykłe odtwarzanie bezpośrednie pozostaje bez zmian.
2. Poprawiono pozycję kursora podczas wielogłosowego czytania dialogów. Znaczniki głosu automatycznie wstawiane przez Sonarpad dla dialogów są teraz traktowane wyłącznie jako metadane odtwarzania, a nie jako znaki w edytorze, dzięki czemu po F4 lub F6 kursor nie przeskakuje przed rzeczywisty tekst. Czytanie jednym głosem oraz jawne znaczniki <voice> w dokumentach pozostają bez zmian.

Audiodeskrypcja z AI
1. Dodano pole wyboru „Pokaż klucz API” bezpośrednio po polu klucza API Gemini. Domyślnie jest wyłączone; po włączeniu tymczasowo pokazuje cały klucz, aby można było sprawdzić, czy został wklejony w całości. Po ponownym otwarciu okna klucz jest znów ukryty.

Podcasty i Wikipedia
1. Po zapisaniu nagrania podcastu Sonarpad pyta teraz, czy otworzyć folder zawierający zapisany plik, tak jak po zapisywaniu multimediów z YouTube/streamingu.
2. Gdy kolejny artykuł z Wikipedii jest importowany do edytora zawierającego już tekst, nowy artykuł jest teraz dodawany na końcu zamiast na początku. Kursor zostaje ustawiony na początku nowo zaimportowanego artykułu.


Wersja 0.9.2 – 2026-09-02

Audiodeskrypcja AI
1. Naprawiono problem, który mógł powodować błąd audiodeskrypcji AI podczas końcowego eksportu do MP3 w filmach z dźwiękiem wielokanałowym, na przykład 5.1. Sonarpad automatycznie miksuje teraz dźwięk wielokanałowy do stereo tylko wtedy, gdy jest to wymagane do kodowania MP3, bez zmiany eksportu mono lub stereo.
2. Przy uruchamianiu audiodeskrypcji AI dla filmu zawierającego wiele ścieżek audio Sonarpad pyta teraz przed rozpoczęciem przetwarzania, której ścieżki użyć. Dostępne pole kombi można zmieniać strzałkami; OK uruchamia audiodeskrypcję z wybraną ścieżką, a Anuluj zamyka okno audiodeskrypcji i przywraca fokus do edytora Sonarpad.

YouTube i strumieniowanie
1. Naprawiono problem, przez który uruchomienie audiodeskrypcji AI dla filmu znajdującego się na stronie 2 lub dalszej playlisty albo kanału YouTube mogło ponownie otworzyć okno wyboru YouTube i odebrać fokus oknu audiodeskrypcji. Sonarpad zamyka teraz selektor prawidłowo, bez powrotu do poprzednich stron.
Wersja 0.9.1 – 2026-09-01

Pobieranie z YouTube
• Naprawiono problem, przez który okna postępu pobierania z YouTube/streamingu mogły wielokrotnie wracać na pierwszy plan po przełączeniu do innej aplikacji za pomocą Alt+Tab. Pobieranie działa teraz w tle bez przejmowania fokusu.
• Ulepszono dostępność postępu pobierania. Po powrocie do okna postępu czytniki ekranu mogą odczytać bieżący stan i procent. W przypadku playlist Sonarpad podaje także numer bieżącego elementu, łączną liczbę elementów i tytuł.
• Naprawiono fałszywe zgłoszenia zawieszenia przez watchdog podczas długich pobrań i konwersji, gdy okno postępu nadal odpowiadało.
• Do pobierania playlist dodano pole kombi Format. Z listy filmów można nacisnąć Tab i wybrać MP4, MP3, M4A, OPUS, OGG, WAV lub FLAC przed rozpoczęciem pobierania wielu elementów.
• Przeorganizowano zapisywanie multimediów strumieniowych. Format i jakość są teraz wybierane podczas zapisywania, a nie w początkowym oknie wyszukiwania strumienia. „Zapisz multimedia” otwiera jedno okno Formatu i Jakości, a pobieranie playlist udostępnia oba pola kombi.

Audiodeskrypcja z AI
• Naprawiono problem, który mógł uniemożliwiać uruchomienie audiodeskrypcji z AI dla niektórych filmów MKV. Sonarpad lepiej obsługuje teraz filmy z nieregularnymi lub brakującymi znacznikami czasu.

Wersja 0.9.0 – 2026-08-31

Audiodeskrypcja z AI — nowa główna funkcja
• W Narzędzia > Multimedia dodano „Utwórz audiodeskrypcję z AI”. Sonarpad analizuje dźwięk, wyszukuje miejsca bez dialogów, generuje opisy za pomocą Gemini i używa dostępnych już silników mowy, nie mówiąc ponad dialogami.
• Ulepszono synchronizację między tym, co dzieje się w filmie, a opisami oraz dodano automatyczne sprawdzanie czasów generowanych przez Gemini.
• „Włącz rozszerzone pauzy” jest domyślnie wyłączone. Opcję można włączyć w materiałach z dużą liczbą dialogów lub niewielką ilością wolnego miejsca, aby umożliwić wstawianie dłuższych opisów.
• Sonarpad może próbować rozpoznawać postacie i używać ich imion. Katalogi postaci można zachowywać między odcinkami serialu, aby poprawić ciągłość.
• Projekt można zapisać, później poprawić opisy i ponownie wyeksportować bez ponownego generowania wszystkiego przez Gemini.
• Jeśli proces zostanie przerwany, Sonarpad zachowuje postęp i pozwala kontynuować audiodeskrypcję. Po wyczerpaniu limitu Gemini można poczekać, zmienić model lub przerwać bez utraty ukończonej pracy.
• Okno pozwala wybrać język, poziom szczegółowości, model Gemini, silnik mowy i głos oraz zapamiętuje używane ustawienia.
• Moduł jest dostępny we wszystkich 17 językach Sonarpada. Podczas generowania interfejs pokazuje tylko postęp, bieżący stan i Anuluj; po zakończeniu MP3 można otworzyć bezpośrednio w wewnętrznym odtwarzaczu.

E-booki i dokumenty
• Dodano import Kindle bez DRM w formatach MOBI, AZW i AZW3, z tekstem i rozdziałami dostępnymi w edytorze i indeksie.
• Dodano obsługę DAISY 2.02 i DAISY 3. Audiobooki DAISY korzystają z wewnętrznego odtwarzacza Sonarpada i respektują nawigację oraz granice rozdziałów.
• Kindle i DAISY są importowane bez nadpisywania oryginalnego pliku; Kindle chronione DRM są wyraźnie odrzucane.
• Poprawiono „Zapisz jako” dla EPUB: po wybraniu TXT lub innego formatu używane jest właściwe rozszerzenie, a oryginalny EPUB pozostaje powiązany z otwartym dokumentem.

RSS i artykuły
• Dodano wielokrotny wybór artykułów RSS, aby usuwać kilka artykułów w jednej operacji.
• RSS obsługuje teraz prawdziwe foldery zachowywane podczas importu i eksportu OPML, w tym puste foldery.
• Kanały można porządkować wewnątrz bieżącego folderu poleceniami Przenieś w górę, Przenieś w dół, Przenieś na początek, Przenieś na koniec i Przenieś na pozycję.

Dostępność, przewodniki i interfejs
• Przewodniki Sonarpada zostały uporządkowane i wyposażone w spis treści oraz pełny przewodnik po audiodeskrypcji z AI.
• Naprawiono problem niemieckiego tłumaczenia, który mógł uniemożliwiać wyświetlanie okien Otwórz, Zapisz jako i innych okien wyboru plików.

Głosy i języki
• Katalog głosów Google TTS do pobrania został rozszerzony ze 104 do 156 pakietów i z 53 do 81 wariantów językowych.
• Dodano nowe pakiety Google TTS oraz zlokalizowane nazwy kolejnych języków w całym interfejsie.

Wersja 0.8.4 – 2026-07-24

Edycja dokumentów EPUB
• Sonarpad potrafi teraz nie tylko otwierać dokumenty EPUB, lecz także je edytować i ponownie zapisywać w formacie EPUB z zachowaniem oryginalnego formatowania, spisu treści, przypisów, obrazów, arkuszy stylów, metadanych i odsyłaczy wewnętrznych.
• Format EPUB jest dostępny w oknie „Zapisz jako” dla dokumentów otwartych z pliku EPUB. Podczas zapisu aktualizowany jest wyłącznie zmieniony tekst, a struktura książki pozostaje nienaruszona.

Niezawodność audiobooków
• Naprawiono sporadyczny problem, przez który po pięciu nieudanych próbach Google TTS jednostka syntezy była po cichu pomijana, a w gotowym audiobooku mogło brakować fragmentu tekstu.
• Jednostki Google są teraz ponawiane aż do powodzenia lub anulowania przez użytkownika. Uruchamianie procesów jest rozłożone w czasie, aby ograniczyć tymczasowe konflikty z Chrome i plikami; Sonarpad przerywa też tworzenie zamiast zapisywać audiobook z brakującym segmentem.
• Audiobooki Edge ponawiają teraz bez stałego limitu tymczasowe błędy sieci, WebSocket, przekroczenia czasu, ograniczenia usługi i nieprawidłowego dźwięku, aż do powodzenia lub anulowania przez użytkownika, również przy głosach mieszanych i podziale według czasu. SAPI4 i SAPI5 zachowują adaptacyjne, ograniczone próby; jeśli segment nadal się nie powiedzie, Sonarpad przerywa proces bez zapisywania niekompletnego audiobooka.

Nawigacja w bibliotekach cyfrowych
• Wyniki LibriVox, Internet Archive i Project Gutenberg korzystają teraz z nawigacji stronami, tak jak YouTube: „Przejdź do poprzednich wyników” znajduje się na początku listy, a „Przejdź do następnych wyników” na końcu.
• Poprawiono przełączanie fokusu w LibriVox: po otwarciu książki lub rozdziału fokus NVDA nie jest już przenoszony do głównego edytora przed otwarciem następnej listy lub odtwarzacza.
• Dodano ochronę fokusu podczas wyszukiwania i wczytywania książek LibriVox: zlokalizowane okno ładowania pozostaje na pierwszym planie przez cały czas wykonywania żądania, dzięki czemu fokus NVDA nie przechodzi do Wiersza polecenia, Windows Terminal ani innej aplikacji.

Pobieranie playlist YouTube
• Do playlist YouTube dodano dostępne polecenie wielokrotnego wyboru, które pozwala wskazać filmy do pobrania bez zmiany dotychczasowego polecenia „Zapisz multimedia” dla aktualnie odtwarzanego elementu.
• Wybrane elementy są pobierane kolejno w formacie i jakości wybranych podczas otwierania playlisty, otrzymują numerowane nazwy zachowujące pierwotną kolejność i są zapisywane w osobnym folderze wewnątrz skonfigurowanego folderu Multimedia.
• Okno zawiera polecenia „Zaznacz wszystko” i „Odznacz wszystko”, ogłasza liczbę wybranych elementów, pozwala anulować z zachowaniem ukończonych plików i wyraźnie informuje o elementach, których nie udało się pobrać.
• Elementy playlisty są teraz natywnymi polami wyboru: czytniki ekranu automatycznie ogłaszają tytuł, typ kontrolki i stan zaznaczenia, bez dodawania słów do widocznego tytułu i bez wymuszonych komunikatów głosowych.

Wersja 0.8.3 – 2026-07-23

Tryb ciemny
• Dodano tryb ciemny, który można włączyć w menu Widok i który jest zapisywany w preferencjach użytkownika.
• Ciemny motyw obejmuje edytor, menu, okna dodatkowe i główne elementy sterujące, a kolory tekstu są dostosowywane w celu zachowania czytelności i dostępności.

Język niemiecki
• Dodano język niemiecki jako pełny język interfejsu, wybierany w Opcjach.
• Wiadomości i RSS, sprawdzanie pisowni, kalendarz i wszystkie cytaty, darowizny, przewodnik oraz dziennik zmian są w całości dostępne po niemiecku.

Portugalski brazylijski i Wiadomości Google
• Dodano portugalski brazylijski jako pełny język interfejsu, niezależny od portugalskiego używanego w Portugalii i dostępny w Opcjach.
• Cały interfejs, kalendarz i wszystkie cytaty, sprawdzanie pisowni, darowizny, podręcznik oraz dziennik zmian są dostępne po portugalsku brazylijskim.
• Wiadomości Google obsługują teraz lokalizację brazylijską, kategorie brazylijskie i osobne domyślne brazylijskie źródła RSS.
• Gdy kanał je udostępnia, powiązane źródła tej samej wiadomości są wyświetlane jako dostępne elementy podrzędne w drzewie.

LibriVox
• Zoptymalizowano wyszukiwanie w LibriVox, aby uniknąć nadmiernej liczby zapytań do usługi i zawieszania interfejsu. Usunięto rozległe skanowanie katalogu, zmniejszono liczbę prób i wprowadzono krótsze limity czasu.

Synteza mowy
• Sekwencje trzech lub większej liczby kropek są teraz normalizowane przed odczytem, dzięki czemu niektóre głosy nie wypowiadają „kropka kropka” ani nie tworzą fragmentów złożonych wyłącznie ze znaków interpunkcyjnych.

Powiązane artykuły Google News
• Dla każdej wiadomości, jeśli są dostępne, wyświetlane są teraz powiązane artykuły, czyli inne artykuły opisujące tę samą wiadomość. Aby je przeczytać, wystarczy rozwinąć artykuł główny, gdy Sonarpad poinformuje, że dostępne są powiązane artykuły. Jeśli ktoś nie chce rozwijać tej sekcji, wystarczy nacisnąć Enter na artykule głównym i przeczytać wiadomość tak jak dotychczas.
• Powiązane artykuły korzystają teraz z tego samego systemu przeczytane/nieprzeczytane co artykuły główne, wraz z dostępnymi komunikatami, datą i godziną, zapisywaniem stanu oraz jego zachowaniem po odświeżeniu źródeł lub ponownym uruchomieniu Sonarpada.

Zapowiedzi w częściach audiobooków
• W opcjach dźwięku dodano pole kombi „Zapowiedź na początku każdej części”. W audiobookach podzielonych na wiele plików każda część może rozpoczynać się bez zapowiedzi albo od tytułu książki, tytułu i numeru części, nazwy pliku lub nazwy pliku i numeru części.

Wersja 0.8.2 – 2026-07-17

Biblioteki cyfrowe i audiobooki
• Dodano Project Gutenberg z wyszukiwaniem według tytułu lub autora oraz wyborem języka.
• Książki EPUB z Project Gutenberg są pobierane do folderu Dokumenty\Sonarpad\Documents; po zakończeniu pobierania Sonarpad pyta, czy od razu otworzyć książkę w edytorze.
• Dodano Internet Archive do wyszukiwania i słuchania kolekcji audio, w tym dawnych audycji radiowych, przemówień i muzyki na żywo.
• Dodano LibriVox do wyszukiwania audiobooków według tytułu lub autora oraz bezpośredniego odtwarzania rozdziałów za pomocą tego samego odtwarzacza co podcasty.
• Trzy nowe funkcje są dostępne w menu Narzędzia, a po włączeniu grupowania menu również w sekcji Czytanie.

Długie transkrypcje audio
• Naprawiono transkrypcję długich plików audio: dźwięk jest teraz automatycznie dzielony na 15-minutowe części, transkrybowany kolejno, a następnie ponownie łączony, co zapobiega błędom występującym przy długich nagraniach.

YouTube
• Najbardziej przydatne działania, które wcześniej były dostępne dopiero po otwarciu filmu z YouTube i wejściu do menu Odtwarzanie, są teraz dostępne także bezpośrednio w menu kontekstowym tego samego filmu, na przykład „Transkrybuj bieżące audio”, „Utwórz audiodeskrypcję z pomocą AI” i „Zapisz media”, co ułatwia obsługę.
• Dodano polecenie „Kopiuj link”, dostępne także pod skrótem Ctrl+C, które kopiuje do schowka adres URL wybranego filmu, playlisty lub kanału YouTube.

Wersja 0.8.1 – 2026-07-16

Synteza mowy Google
• Naprawiono uruchamianie Google TTS w systemach Windows, w których połączenia zaakceptowane przez wewnętrzny serwer przeglądarki dziedziczyły nieblokujący tryb gniazda, powodując błąd 10035 i uniemożliwiając działanie pobranych głosów.
• Sonarpad czeka teraz na pełne załadowanie silnika WASM w Chrome lub Edge przed odsłuchem głosu albo czytaniem klawiszem F5, zapobiegając błędowi „Chrome WASM TTS engine was not loaded”.
• Ukryta przeglądarka wyłącza tłumaczenie stron i dostępność procesu renderującego, aby nie ogłaszać opcji takich jak „Przetłumacz stronę” i nie zakłócać poleceń czytania.
• Panel „Głosy w edytorze” pokazuje teraz przycisk „Zarządzaj głosami Google...” po wybraniu silnika Google i od razu odświeża listę zainstalowanych głosów po zamknięciu menedżera.
• Ostrzeżenia o zależnościach wyświetlane podczas usuwania pakietów głosów Google są teraz przetłumaczone na wszystkie języki interfejsu.

Obsługa aktualizacji
• Po automatycznej aktualizacji okno zakończenia z listą zmian otwiera się po początkowym przywróceniu fokusu i pozostaje na pierwszym planie, zamiast pojawiać się dopiero po naciśnięciu klawisza Tab.

Dokumenty PDF
• Naprawiono pliki PDF, których osadzony tekst zawierał znaki NUL i był obcinany przy pierwszym z nich podczas wczytywania do edytora.
• Gdy pdf-extract zwróci osadzone znaki NUL, Sonarpad ponawia ekstrakcję przez PDFium; pozostałe znaki NUL są usuwane przed przekazaniem tekstu do kontrolek Windows, dzięki czemu dalsza część dokumentu zostaje zachowana.

Dostępność menu
• Usunięto obliczanie mnemonik w czasie działania programu: klawisze dostępu są teraz zapisane jawnie we wszystkich 15 tłumaczeniach interfejsu i pozostają takie same przy każdym uruchomieniu.
• Sprawdzono wszystkie stałe pozycje głównych menu i podmenu, w tym Odtwarzanie, czcionki, Zapisz obraz oraz Pokaż indeks EPUB; brakujące lub powtarzające się mnemoniki wśród pozycji tego samego poziomu poprawiono bezpośrednio w tłumaczeniach.
• Testy automatyczne wyłącznie weryfikują teraz tłumaczenia i zgłaszają błąd, gdy mnemoniki brakuje, jest nieprawidłowa lub powtórzona; nie zmieniają już etykiet podczas działania programu.
• W wyjątkowo rozbudowanych menu, gdy przetłumaczone etykiety nie zawierają wystarczającej liczby różnych znaków, wyświetlany jest jawny numeryczny klawisz dostępu w standardowej postaci Windows „(&1)”.

Wersja 0.8.0 – 2026-07-15

Słownik internetowy
• Dodano język niemiecki do internetowego słownika Wiktionary.
• Niemieckie definicje i synonimy są teraz prawidłowo rozpoznawane zgodnie ze strukturą niemieckiego Wiktionary.

Niezawodność audiobooków SAPI5
• Tworzenie audiobooków SAPI5 nadal używa do 12 równoległych procesów roboczych, gdy wybrany głos generuje prawidłowe wyniki.
• Każda część jest sprawdzana na podstawie rozmiaru pliku, szacowanego czasu i ostrożnego porównania z przypisanym tekstem.
• Brakujące lub podejrzane części są automatycznie generowane ponownie przy stopniowo mniejszej współbieżności: 12, 8, 6, 4, 2 i na końcu 1 proces. Powtarzane są tylko problematyczne części.
• Niezawodny limit jest zapamiętywany oddzielnie dla każdego głosu SAPI5, bez spowalniania głosów działających poprawnie z 12 procesami.
• Kontrola końcowa zapobiega cichemu zaakceptowaniu pliku MP3 znacznie krótszego niż wygenerowane części.
• Szczegóły są zapisywane w `sapi5_audiobook_diagnostic.log`.
• Każda jednostka syntezy SAPI5 działa teraz w oddzielnym, ukrytym procesie Sonarpad. Jeśli głos firmy trzeciej ulegnie awarii, zamykany jest tylko ten worker, a główna aplikacja pozostaje uruchomiona.
• Podczas tego samego tworzenia audiobooka niedokończone części są natychmiast ponawiane z następnym niższym poziomem współbieżności; części już zweryfikowane zostają zachowane.
• Odzyskiwanie przy następnym uruchomieniu pozostaje dodatkowym zabezpieczeniem tylko na wypadek przerwania głównej aplikacji lub pracy komputera.

Procesy audiobooków SAPI4
• Liczba procesów SAPI4 wybrana przez użytkownika jest teraz respektowana do technicznego maksimum 64; usunięto wcześniejszy ukryty limit 16.
• Rzeczywista liczba jest zmniejszana tylko wtedy, gdy audiobook zawiera mniej jednostek pracy niż zażądano.
• Jeśli jeden lub więcej procesów mostka SAPI4 ulegnie awarii, ukończone części są zachowywane, a tylko nieudane jednostki są automatycznie ponawiane z coraz mniejszą współbieżnością.
• Sonarpad sprawdza teraz kod zakończenia mostka SAPI4 i odrzuca puste lub nieprawidłowe części audio.

Konfiguracja proxy
• W ustawieniach sieci dodano osobne pole portu proxy.
• Port można podać niezależnie od adresu proxy; jest sprawdzany w zakresie od 1 do 65535 i prawidłowo zastępuje port już obecny w adresie URL.

Wyszukiwanie radia według języka i kraju
• Filtry Język i Kraj są teraz uzupełniane wszystkimi pozycjami dostępnymi w katalogu Radio Browser i nie są już ograniczone do stałej listy.
• Nazwy języków są teraz rozpoznawane również wtedy, gdy Radio Browser podaje je w innym alfabecie, w formie rodzimej, jako skróty lub połączenia kilku języków, i są wyświetlane w aktualnym języku interfejsu. Wartości, które nie są rzeczywistymi językami, takie jak liczby, gatunki muzyczne, kraje lub ogólne opisy, są odfiltrowywane.
• Katalog jest aktualizowany w tle, a w razie niedostępności Radio Browsera nadal działa lista zapasowa.
• Zduplikowane pozycje językowe Radio Browsera, które po tłumaczeniu mają identyczną nazwę, są teraz łączone w jeden element listy, co zapobiega cichym przejściom w czytnikach ekranu.

Najważniejsze ulepszenie: synchronizacja czytania i kursora
• Synchronizacja odczytu głosowego z ruchem kursora została znacząco ulepszona dla wszystkich obsługiwanych silników mowy.
• Po włączeniu opcji „Przesuwaj kursor podczas czytania” Sonarpad korzysta ze wspólnego systemu postępu dla Microsoft Edge Neural, Google TTS, SAPI4, SAPI5 i OneCore.
• Kursor dokładniej podąża za faktycznie wypowiadanym tekstem, z bardziej spójnym podziałem zdań i ich fragmentów.
• Znacznie ograniczono wyprzedzanie, opóźnienia, nieregularne skoki i różnice między silnikami mowy.
• Prawidłowa pozycja jest lepiej zachowywana po wstrzymaniu, wznowieniu, wyszukiwaniu w dokumencie lub zmianie silnika.

Oddzielne pliki podczas nagrywania podcastu
• Dodano opcję „Zapisz mikrofon oraz dźwięk systemowy lub dźwięk aplikacji w osobnych plikach”.
• Podczas jednoczesnego nagrywania mikrofonu i innego źródła Sonarpad może utworzyć jeden plik tylko z mikrofonem oraz drugi z dźwiękiem systemowym, jedną aplikacją lub wybranymi aplikacjami.
• Rozdzielanie źródeł jest dostępne w MP3 i WAV.
• Gdy opcja jest wyłączona, nadal tworzony jest jeden zmiksowany plik.
• Oddzielne pliki ułatwiają regulację głośności, usuwanie szumów i późniejszy montaż podcastów, wywiadów i poradników.

Planowane nagrania radiowe
• Nagrania radiowe można teraz planować z wyprzedzeniem.
• Dla każdego nagrania można wybrać stację, dzień, godzinę i minutę rozpoczęcia oraz czas trwania.
• Dostępny jest własny czas trwania od 1 do 1440 minut.
• Nagranie może zostać wykonane raz, codziennie lub co tydzień.
• Okno wyraźniej pokazuje trwające i zaplanowane nagrania, planowaną datę i godzinę, czas trwania oraz czas pozostały do rozpoczęcia.
• Harmonogram zadań Windows może uruchomić nagranie automatycznie, nawet gdy Sonarpad nie jest otwarty.

Kalendarz
• Dodano kompletny kalendarz dostępny z klawiatury.
• Można przeglądać poprzednie i następne dni, szybko wrócić do dzisiaj oraz sprawdzić święta i rocznice.
• Dodano patrona dnia i cytat dnia, które można przeczytać, odsłuchać lub skopiować.
• Przypomnienia można tworzyć, edytować, usuwać, odkładać i oznaczać jako wykonane.
• Powiadomienia mogą pojawiać się o dokładnej godzinie lub wcześniej i korzystać z harmonogramu Windows również przy zamkniętym Sonarpadzie.

Pogoda
• Dodano sekcję prognozy pogody.
• Można wyszukać miasto i szybko ponownie otworzyć ostatnio sprawdzane miejsca.
• Dostępne są bieżące warunki, temperatura, minimum i maksimum, wilgotność, prawdopodobieństwo opadów oraz prognoza na kolejne dni.
• Można wybrać stopnie Celsjusza, Fahrenheita lub tryb automatyczny.

Filmy w kinach
• Dodano sekcję z filmami aktualnie wyświetlanymi w kinach i nadchodzącymi premierami.
• Dostępne są wyszukiwanie po tytule, opis fabuły, data premiery i odtwarzanie zwiastuna.

Synteza mowy Google
• Dodano Google TTS do czytania dokumentów i tworzenia audiobooków.
• Dodano menedżer głosów pozwalający je wyświetlać, filtrować według języka, pobierać i usuwać niepotrzebne głosy.
• Można regulować szybkość, głośność i wysokość głosu.
• Wysokość głosów Google Natural jest stosowana bezpośrednio przez silnik, co daje bardziej naturalny i stabilny rezultat.
• Poprawiono szybkość reakcji i niezawodność Google TTS, dostosowując limity czasu syntezy do wybranej szybkości.
• Ograniczono zbędne oczekiwanie i poprawiono obsługę błędów i przerwań.

Spis treści EPUB
• Sonarpad rozpoznaje teraz spis treści osadzony w książkach EPUB.
• Jego obecność jest ogłaszana i można go otworzyć z menu Widok.
• Rozdziały i podrozdziały są wyświetlane hierarchicznie.
• Naciśnięcie Enter natychmiast przenosi do wybranego miejsca.

Wiadomości i źródła RSS
• Rozszerzono sekcję Wiadomości o nowe narzędzia wyszukiwania i organizacji.
• Dodano wybór języka wiadomości.
• Można przeszukiwać źródła RSS i czytać wiadomości ze swojego miasta.
• Źródła społeczności można przeglądać, dodawać do własnej kolekcji i przesyłać społeczności Sonarpad.

Nagrywanie podcastów
• Można nagrywać tylko mikrofon, cały dźwięk systemowy, jedną aplikację, wiele wybranych aplikacji albo mikrofon i aplikacje jednocześnie.
• Można wybrać urządzenie mikrofonowe i źródło dźwięku, osobno regulować głośność i obserwować poziomy w czasie rzeczywistym.
• Dodano wstrzymywanie i wznawianie, zapis MP3 lub WAV, wybór bitrate MP3 i folderu docelowego.
• Podczas nagrywania komputer może pozostać aktywny.

Radio
• Sekcja Radio została gruntownie przeorganizowana.
• Stacje można wyszukiwać według nazwy lub dowolnego tekstu, języka, kraju, miasta, gatunku muzycznego lub kategorii.
• Poprawiono zarządzanie ulubionymi i dodano szybkie zerowanie wszystkich filtrów.
• Stacje można przesyłać społeczności Sonarpad.
• Dodano nagrywanie na żywo, tryb „Nagrywaj i odtwarzaj”, listę nagrań oraz ich zarządzanie i usuwanie.
• Nagrania radiowe są przechowywane we własnym folderze w głównym katalogu nagrań.

Odtwarzanie multimediów
• Znacząco poprawiono stabilność odtwarzacza multimedialnego.
• Naprawiono problem mogący blokować mpv i poprawiono komunikację z odtwarzaczem.
• Ulepszono otwieranie różnych typów plików multimedialnych.
• Sonarpad zapamiętuje teraz używany poziom głośności.
• Poprawiono obsługę strumieni i nagrań.
• Naprawiono otwieranie plików z Windows przez dwukrotne kliknięcie lub „Otwórz za pomocą”.

Dokumenty PDF
• Dodano rozpoznawanie pól formularzy w PDF.
• Sonarpad może odnaleźć pola do wypełnienia, przedstawić je w dostępnej postaci tekstowej, umożliwić edycję i zapisać dane w PDF.
• Poprawiono obliczanie pozycji kursora podczas czytania, szczególnie przy znakach wielobajtowych i złożonych strukturach.

Dostępność i klawiatura
• Poprawiono standardowe polecenia edycji w całym programie.
• Kopiuj, wytnij, wklej, zaznacz wszystko, cofnij i ponów są prawidłowo wysyłane do pola z fokusem, także w oknach dodatkowych i dialogach.
• Naprawiono problem z aktualizacją monitorów brajlowskich.
• Poprawiono zarządzanie fokusem i wybór języka w Wikipedii.
• Dodano grupowanie funkcji menu Narzędzia według kategorii.
• Dodano konfigurowalne działania do szybkiego otwierania Kalendarza, Pogody i Filmów w kinach.

Audiobooki
• Poprawiono tworzenie audiobooków przy otwartych oknach dialogowych lub modalnych.
• Obsługa postępu jest bardziej odporna i ignoruje nieaktualne aktualizacje dźwięku.
• Google TTS może być używany również do tworzenia audiobooków z regulacją szybkości, głośności i wysokości.

Sztuczna inteligencja
• Domyślny model Gemini został zaktualizowany do `gemini-3.5-flash`.

Poprawki ogólne
• Naprawiono kilka zawieszeń podczas odtwarzania przez mpv.
• Naprawiono otwieranie niektórych plików audio i wideo.
• Poprawiono zarządzanie poleceniami wysyłanymi do odtwarzacza.
• Naprawiono przywracanie kursora podczas czytania.
• Poprawiono stabilność tworzenia audiobooków.
• Poprawiono ogólną obsługę multimediów, RSS, radia i EPUB.

Wersja 0.7.1 – 2026-05-13

Nowości i ulepszenia
• Utworzono oficjalną stronę sonarpad.com, nowe miejsce odniesienia do śledzenia najnowszych informacji, pobierania najnowszej wersji programu, czytania komentarzy odwiedzających oraz, w przyszłości, słuchania wszystkich podcastów Sonarpad. Do menu Pomoc dodano także pozycję „Odwiedź sonarpad.com”, która pozwala szybko otworzyć oficjalną stronę.
• Naprawiono problem, przez który pliki z akcentami lub znakami specjalnymi powodowały błąd podczas uruchamiania transkrypcji głosowej.
• Od teraz w menu Widok pozycje takie jak Automatyczne zawijanie wierszy i Pokazuj wideo podczas odtwarzania zawsze będą pokazywać prawidłowy stan, włączony lub wyłączony.
• Ulepszono wyszukiwanie w YouTube, umożliwiając powrót klawiszem Esc do poprzedniej strony lub ekranu.
• Dodano wstępną kontrolę sprawdzającą, czy wideo można odtworzyć. Ulepszono także odtwarzanie: Sonarpad może teraz odtwarzać również filmy lub playlisty oznaczone jako mix, które wcześniej nie były odtwarzane.
• Ulepszono obsługę automatycznych zakładek. Wcześniej, jeśli opcja Automatyczne zakładki była włączona, a następnie wyłączona, te zakładki pozostawały; teraz program prawidłowo je ignoruje, dopóki opcja nie zostanie ponownie włączona. Ponadto po dojściu do końca pliku multimedialnego zakładka zostanie automatycznie usunięta.
• Ulepszono obsługę tagów przy aktywnych dialogach. Sonarpad poprawnie obsługuje teraz obie funkcje, pozwalając wstawiać tagi także wtedy, gdy opcja dialogów jest aktywna.
• Ulepszono ustawienia głosu, wyraźnie rozdzielając każdy silnik, dzięki czemu regulacja jest dokładniejsza. Profile głosu prawidłowo zachowują ustawienia dla każdego pojedynczego silnika: Edge, Sapi5 i Sapi4.
• Dodano tag do wstawiania pauz, bezpośrednio z opcji lub z panelu głosów po naciśnięciu Tab z edytora. Dostępne opcje to: 250 ms, 500 ms, 1 sekunda, 2 sekundy lub czas niestandardowy.
• Naprawiono zachowanie podczas odtwarzania filmu z YouTube i uruchamiania transkrypcji. Teraz po powrocie za pomocą Alt+Tab fokus będzie prawidłowo ustawiony na przycisku Anuluj aktywnej transkrypcji.
• Transkrypcje są teraz automatycznie zapisywane po zakończeniu procesu.
• Ulepszono import z Wikipedii. Można wybrać, czy czytać tylko jedną sekcję i potem z artykułu wrócić do wyszukiwania klawiszem Esc, czy zaimportować cały artykuł. Można także wybrać język Wikipedii.
• Dodano sekcję radia z całego świata, w której można wyszukiwać stacje radiowe według kraju, języka i gatunku. Można także dodawać lokalne stacje radiowe do bazy danych Sonarpada, aby inni użytkownicy również mogli ich słuchać. Radio można też dodać do ulubionych.
• Dodano sekcję tras do obliczania przejazdów z wyborem środka: pieszo, rowerem, samochodem lub na wózku inwalidzkim. Można wybrać trasę najkrótszą lub najszybszą oraz zdecydować, czy pokazywać mijane gminy. Po zaimportowaniu trasy można także zapisać mapę wizualną z menu Plik, Zapisz obraz.
• Dodano pozycję Drukuj w menu Plik. Sonarpad będzie drukował pliki TXT własnym systemem, a dla innych plików, takich jak DOCX, PDF i podobne, użyje powiązanego programu, aby jak najlepiej zachować oryginalny układ.
• Zintegrowano w Sonarpadzie usługę tłumaczenia dla każdego dokumentu, dostępną z menu kontekstowego edytora. Użytkownik może korzystać z bezpłatnych usług DeepL i Google Translate bez podawania żadnego klucza API; po wpisaniu klucza API Gemini może tłumaczyć za pomocą Gemini.
• W menu tłumaczenia użytkownik może wybrać język docelowy. Menu automatycznie zmienia kolejność: jeśli użytkownik najpierw wybierze angielski, potem francuski, a potem włoski, te trzy opcje będą widoczne na górze menu języków.
• Jeśli użytkownik wpisze swój klucz API Gemini, będzie mógł także korzystać z funkcji Streść tekst, również dostępnej w menu kontekstowym, aby streszczać dowolny artykuł.
• Do menu Odtwarzaj, widocznego podczas odtwarzania pliku multimedialnego, dodano menu do dzielenia bieżącego medium. Działa z MP3, MP4 i innymi formatami, dzieląc według liczby części albo według czasu trwania każdej części.

Wersja 0.7.0 – 2026-04-25

Nowości
• Dodano obsługę odtwarzacza mpv do odtwarzania strumieniowego. Filmy z YouTube i obsługiwanych stron są teraz odtwarzane natychmiast; jeśli użytkownik zdecyduje się je zachować, są pobierane jak wcześniej. W przypadku transkrypcji treści strumieniowych są one najpierw pobierane, a następnie transkrybowane. Odtwarzacz mpv jest również używany do otwierania lokalnych plików wideo oraz obsługi napisów, co zapewnia lepszą kompatybilność z wieloma formatami.
• Ulepszono nagrywanie podcastów z dźwięku systemowego: teraz można wybrać nagrywanie całego dźwięku systemowego, jednej aplikacji albo wielu aplikacji jednocześnie. Ta opcja jest zintegrowana ze zwykłym nagrywaniem, więc nadal można osobno włączać lub wyłączać mikrofon.
• Dodano język hindi. Przetłumaczono interfejs oraz dodano RSS, dziennik zmian i przewodnik Sonarpad.
• Dodano opcję w zakładce Edytor, która zawsze przenosi kursor na początek wiersza przy użyciu strzałek w górę i w dół.
• Dodano opcję w menu "Konwertuj audio", umożliwiającą konwersję audio do M4B.

Poprawki
• W komentarzach YouTube otwieranych z poziomu „Odtwarzaj dźwięk ze strumienia...” Sonarpad ładuje teraz na początku tylko pierwszych 50 komentarzy głównych, zawsze wraz ze wszystkimi odpowiedziami do tych komentarzy, i dodaje na końcu pozycję umożliwiającą wczytanie wszystkich komentarzy na żądanie.
• Zakładki są teraz wyświetlane i obsługiwane według pozycji zarówno w dokumentach tekstowych, jak i w plikach multimedialnych, zamiast według kolejności utworzenia. Jeśli zakładka już istnieje w tej samej pozycji, nie jest dodawana ponownie.
• Dodano opcję w menu Zakładki, która po włączeniu umożliwia automatyczne zarządzanie zakładkami. Podczas odtwarzania pliku lokalnego lub strumieniowego i jego zamknięcia Sonarpad automatycznie ustawi zakładkę na podstawie osiągniętej pozycji, a po ponownym otwarciu pliku wznowi odtwarzanie od tego miejsca. To samo dotyczy plików tekstowych: jeśli tekst zostanie otwarty i kursor zostanie przesunięty, Sonarpad zapamięta tę pozycję po zamknięciu; jeśli rozpocznie się czytanie, zostanie zapisana ostatnio przeczytana fraza, a czytanie zostanie wznowione dokładnie od tego miejsca.
• Do menu Widok dodano pozycję umożliwiającą wyświetlanie renderowania wideo dla plików lokalnych lub strumieniowych. Zawartość wideo jest pokazywana w powiększonym oknie, w którym wszystkie elementy sterujące są ukryte, chyba że zostanie naciśnięty klawisz Alt albo kursor myszy zostanie przesunięty w górną część okna. Dzięki temu użytkownicy słabowidzący powinni otrzymać większą i bardziej wygodną w użyciu zawartość.

Wersja 0.6.9 – 2026-04-08

Poprawki
• Ulepszono działanie funkcji Znajdź w plikach: po otwarciu Przeglądaj folder fokus od razu trafia na listę folderów; po otwarciu wyniku klawiszem Enter wszystkie skróty klawiaturowe nadal działają; klawisz Esc przywraca wcześniej wybrany wynik; a po powrocie przez Alt+Tab fokus trafia do pola wyszukiwania albo na listę wyników, jeśli była otwarta.
• F5 zawsze rozpoczynał czytanie od początku. Zostało to poprawione i czytanie zaczyna się teraz od bieżącej pozycji kursora, z zachowaniem `Shift+F5` i `Ctrl+F5` do przechodzenia do poprzedniego lub następnego zdania.
• Po użyciu Przejdź do wiersza naciśnięcie Esc mogło przenosić fokus poza Sonarpad. Teraz fokus poprawnie wraca do edytora.
• Opcja `Zawijanie wierszy` jest teraz stosowana od razu także do już otwartych dokumentów, bez konieczności ponownego otwierania pliku.

Wersja 0.6.8 – 2026-04-07

Nowości
• Dodano nową pozycję w menu Odtwarzanie, która umożliwia transkrypcję dowolnego pliku audio lub wideo za pomocą Whisper. W Opcjach dostępna jest nowa sekcja „AI i transkrypcja”, w której można wybrać model, włączyć opcjonalną obsługę CUDA dla kart graficznych NVIDIA, zachować oryginalny język oraz włączyć lub wyłączyć znaczniki czasu.
• Dodano w menu Odtwarzanie nową akcję `Transkrybuj bieżący folder`, która transkrybuje wszystkie obsługiwane pliki audio z folderu aktualnie otwartego medium i łączy je w jeden dokument, z dedykowanym oknem postępu, informacją o bieżącym pliku i możliwością anulowania. Można ją też uruchomić skrótem `Alt+Shift+C`.
• Dodano możliwość korzystania z dyktowania głosowego offline, działającego tak samo jak transkrypcja audio. Domyślnie naciśnij `Ctrl+Shift+Spacja`, aby rozpocząć dyktowanie, i naciśnij ten sam skrót ponownie, aby je zakończyć; skrót można dostosować w Opcjach. Od drugiego uruchomienia dyktowanie działa szybciej, ponieważ silnik pozostaje gotowy w pamięci; na komputerach z mniej niż 4 GB RAM to wstępne ładowanie i ponowne użycie są automatycznie wyłączane.
• Dodano nową opcję edytora, domyślnie wyłączoną, dzięki której `Esc` zamyka okno edytora.
• Wyszukiwanie podcastów domyślnie korzysta teraz z `iTunes + Spreaker`, z filtrowaniem duplikatów, gdy ten sam podcast jest dostępny na obu platformach.
• Ulepszono wyszukiwanie i przeglądanie podcastów Apple: wyszukiwanie podcastów, przeglądanie kategorii oraz listy top podcastów w kategoriach korzystają teraz z wybranego kraju katalogu podcastów. W Opcje > RSS / Podcast można pozostawić `Automatycznie`, aby używać kraju systemowego, albo ręcznie wybrać inny kraj.
• Zwiększono limit wyników dla kategorii podcastów Apple. Przy pierwszym otwarciu nadal ładowane jest pierwszych 50 wyników jak dotąd; po wybraniu `Załaduj więcej wyników` Sonarpad pobiera do 200 wyników łącznie (limit Apple) i pozwala przechodzić przez kolejne strony przy zachowaniu płynnego działania.
• Sonarpad jest teraz dostępny także na Macu, choć na razie z częściowym zestawem funkcji. Link do projektu: https://github.com/Ambro86/Sonarpad-Mac

Ulepszenia
• Dodano ponad 50 wybieralnych krajów dla katalogu podcastów, dzięki czemu można teraz wybierać spośród znacznie większej liczby katalogów narodowych.
• Funkcja „Odtwórz dźwięk ze streamingu...” pozwala teraz także wyszukiwać w YouTube po dowolnym tekście albo wkleić link do kanału lub playlisty YouTube, aby wyświetlić ich wyniki.
• Ulepszono sposób wyświetlania wyników w „Odtwórz dźwięk ze streamingu...”: wpisy YouTube zawierają teraz tytuł, czas trwania, kanał i liczbę wyświetleń w czytelniejszym formacie.
• „Odtwórz dźwięk ze streamingu...” obsługuje teraz również komentarze YouTube: można je otworzyć z menu kontekstowego, czytać odpowiedzi i rozwijać wątki komentarzy klawiszem Strzałka w prawo.
• Dodano ulubione YouTube dla kanałów i playlist w „Odtwórz dźwięk ze streamingu...”: można je dodawać z wyników za pomocą menu kontekstowego, otwierać bezpośrednio z listy Ulubione dostępnej po naciśnięciu Tab zaraz za polem adresu URL/zapytania YouTube oraz usuwać później z tej samej listy również przez menu kontekstowe. W wynikach wyszukiwania YouTube menu kontekstowe jest dostępne tylko dla kanałów i playlist.
• Funkcja „Odtwórz dźwięk ze streamingu...” potrafi teraz poprosić o dane logowania, gdy strona wymaga zalogowania. Użytkownik może je wpisać, zapisać dla danej strony i później zarządzać zapisanymi danymi w Opcje > Audio.
• Poprawiono fokus podczas „Odtwórz dźwięk ze streamingu...”, dzięki czemu okno postępu pozostaje stabilniejsze podczas pobierania i konwersji.
• Dodano dwie nowe akcje czytania w menu Głos: `Poprzednie zdanie` i `Następne zdanie`, z konfigurowalnymi skrótami do przeskakiwania podczas czytania tekstu.
• Domyślny skrót dla `Wykonaj plik w interpreterze` to teraz `Ctrl+Shift+F5`, dzięki czemu `Shift+F5` może być domyślnie używany dla `Poprzednie zdanie`.
• Dodano zarządzanie profilami głosu w Opcje > Głos: profile można dodawać, zmieniać nazwę i usuwać.
• Rozszerzono w Opcje > Audio wybór interwału przewijania wstecz podczas odtwarzania o nowe wartości od 1 sekundy do 2 godzin.
• Dodano tłumaczenie rosyjskie dzięki Dmitriyowi.
• Dodano w Opcje > Audio nową możliwość wyboru formatu nazwy części audiobooka: `Tytuł + numer`, `Tylko numer` albo `Numer + tytuł`.
• Dodano w menu kontekstowym artykułów RSS akcję dodawania artykułu do ulubionych.
• Źródło RSS "Ulubione" można usunąć; zostanie utworzone ponownie automatycznie po dodaniu nowego artykułu do ulubionych.
• Dodano skróty klawiaturowe RSS do przenoszenia źródeł w górę/w dół: `Ctrl+Shift+Strzałka w górę` i `Ctrl+Shift+Strzałka w dół`.
• Ulepszono okno RSS, dodając zintegrowany podgląd artykułu, dzięki czemu tekst można przeglądać bezpośrednio tam i szybko przejść do niego klawiszem Tab przed otwarciem pełnego artykułu w edytorze.
• Dodano w RSS wyraźną pozycję „Załaduj więcej wiadomości” na końcu źródła, gdy dostępne są kolejne elementy; naciśnięcie Enter wczytuje następny blok i przenosi fokus na pierwszy nowy artykuł.
• W słowniku głosowym podczas dodawania lub edycji podmiany dostępne jest teraz pole „Uwzględniaj wielkość liter”, które pozwala zdecydować, czy dana podmiana ma rozróżniać wielkie i małe litery.
Poprawki
• Funkcja „Odtwórz dźwięk ze streamingu...” uwzględnia teraz limit pamięci podręcznej podcastów ustawiony już w Opcjach, a ten sam limit obowiązuje także przy odtwarzaniu audiodeskrypcji.
• Poprawiono import z Wikipedii, który na niektórych stronach nie importował poprawnie cytatów obecnych w tekście.
• Ulepszono parser stron internetowych: na niektórych stronach WordPress nie były uwzględniane elementy list oraz niektóre nagłówki sekcji.
• Teraz przy użyciu „Przejdź do wiersza” pole jest wstępnie wypełniane bieżącym numerem wiersza.
• Poprawiono eksport OPML podcastów i RSS, dzięki czemu eksportowane pliki są teraz akceptowane przez iTunes.
• Poprawiono transkrypcję plików multimedialnych: teraz po zamknięciu wygenerowanego dokumentu skrótem Alt+F4 Sonarpad pyta, czy zapisać plik, i proponuje prawidłową nazwę opartą na nazwie transkrybowanego pliku zamiast na pierwszym wierszu tekstu.
• Dodano lokalizowane komunikaty potwierdzenia poprawnego importu i eksportu OPML źródeł RSS oraz podcastów.
• Naprawiono problem, przez który w „Odtwórz dźwięk ze streamingu...” po wpisaniu wyszukiwanej frazy i wybraniu kanału YouTube z wyników program mógł sprawiać wrażenie zawieszonego zamiast otworzyć filmy z tego kanału.
• Naprawiono błąd, przez który lista otwartych plików była wyświetlana w menu Pomoc zamiast w menu Okno.
• Naprawiono przypadek brzegowy streamingu, w którym odtwarzanie mogło się rozpocząć, ale okno „Pobieranie streamingu” pozostawało otwarte, gdy pobrany plik już pasował do formatu docelowego.
• Naprawiono zachowanie konwersji dla streamingu MP3: gdy strumień jest już w MP3 i użytkownik wybierze jawny bitrate MP3 (np. 128 kbps), Sonarpad teraz ponownie koduje do wybranego bitrate zamiast pomijać konwersję.
• Naprawiono skrót `Alt+Shift+L`: teraz poprawnie otwiera listę rozdziałów podczas odtwarzania.
• Naprawiono skrót `Alt+Shift+T`: teraz poprawnie uruchamia „Transkrybuj bieżące audio” zamiast otwierać menu Narzędzia.
• Jeśli dźwięk jest już odtwarzany, Sonarpad przy uruchamianiu transkrypcji automatycznie wstrzymuje to odtwarzanie przed rozpoczęciem pracy.
• Naprawiono problem, przez który po zaimportowaniu artykułu z Wikipedii import mógł się udać, ale tekst artykułu nie był widoczny na ekranie.
• Dodano obsługę osadzonych rozdziałów podcastów z lokalnych plików multimedialnych (np. metadanych rozdziałów MP3): gdy feed/URL nie udostępnia rozdziałów, Sonarpad ładuje je teraz w tle z pobranego pliku, dzięki czemu odtwarzanie startuje od razu, a rozdziały są stosowane, gdy tylko będą gotowe.
• Naprawiono wczytywanie rozdziałów dla pobranych odcinków podcastów otwieranych jako zwykłe lokalne pliki multimedialne: osadzone rozdziały są teraz dostępne także w tym przypadku, a nie tylko po uruchomieniu odtwarzania z okna Podcasty.
• Naprawiono finalizację audiobooków MP3 w SAPI4 i SAPI5: plik końcowy jest teraz poprawnie finalizowany, co zapobiega niepełnym lub niestabilnym plikom po długich eksportach.
• Dodano wyraźny pasek postępu dla etapu finalizacji we wszystkich trybach tworzenia audiobooków: po zakończeniu tworzenia Sonarpad ogłasza i pokazuje osobną fazę finalizacji z widocznym postępem.
• Naprawiono błąd głosów dialogowych: parametry szybkości/tonu/głośności dla pierwszego i drugiego głosu dialogowego są teraz poprawnie stosowane podczas syntezy.
• Ulepszono wykrywanie kodowania dla japońskich plików `.txt`: dodano bezpieczny fallback Shift_JIS/CP932 dla przypadków mojibake, z zachowaniem dotychczasowego działania dla UTF/diakrytyków/chińskiego.
• Wewnętrzna refaktoryzacja bezpieczeństwa: konwersja do implementacji safe tam, gdzie to możliwe, oraz drastyczne zmniejszenie liczby linii kodu unsafe.

Wersja 0.6.7 – 2026-03-02
Ulepszenia
• Ora il programma riesce a gestire Sostituisci tutto in modo massivo su file grandi con un gran numero di sostituzioni.
• Zaktualizowano tłumaczenie polskie dzięki DJ Graco.
• Dodano tłumaczenie litewskie.
• Dodano tłumaczenie chińskie.
• Od teraz częste wersje beta będą publikowane w sekcji Releases projektu, aby użytkownicy mogli testować nowe zmiany przed następną stabilną wersją.
• Dodano skrót `Ctrl+.` do wstawiania znaku wielokropka (…).
• Ulepszono obsługę rozdziałów podcastów: nawigacja rozdziałów działa teraz bardziej niezawodnie, także dla odcinków bezpośrednich/streamingowych, w których rozdziały nie są osadzone w pliku MP3, dzięki wykorzystaniu metadanych rozdziałów z feedu/URL jako fallback, gdy są dostępne. Dodano skróty `Ctrl+Alt+PageUp` (poprzedni rozdział) i `Ctrl+Alt+PageDown` (następny rozdział).
• Zreorganizowano foldery wyjściowe do `Dokumenty\\Sonarpad`: pliki są teraz zapisywane w dedykowanych podfolderach `audiobooks`, `documents`, `recordings` i `media`, z automatyczną migracją ze starych ścieżek.
• Ulepszono obsługę bardzo dużych plików tekstowych (także 60 MB): płynniejsze otwieranie i nawigacja linia po linii, szczególnie z czytnikami ekranu.
• Zaktualizowano przewodniki we wszystkich językach i zasoby lokalizacyjne całej aplikacji, w tym teksty darowizn oraz tłumaczenia instalatora NSIS (nowe ciągi dla chińskiego uproszczonego i litewskiego oraz uzupełnione tłumaczenie ukraińskie setupu).
• Dodano globalną obsługę proxy sieciowego (HTTP/HTTPS oraz SOCKS5/SOCKS5H) dla funkcji online, z walidacją przy zapisie opcji: nieprawidłowe proxy jest sygnalizowane i usuwane automatycznie.
• Dodano nową funkcję w menu Narzędzia: „Odtwórz dźwięk ze streamingu...”, która pozwala wkleić adres URL (YouTube lub bezpośredni link do mediów), wybrać format wyjściowy oraz profil jakości/bitrate (w tym oryginalną jakość/bitrate dla MP3 i MP4) i odtworzyć materiał w odtwarzaczu Sonarpad.
• Dodano obsługę systemowego klawisza multimedialnego Play/Pause (słuchawki/klawiatura): teraz steruje zarówno odtwarzaniem multimediów, jak i pauzą/wznowieniem czytania tekstu (z priorytetem odtwarzacza multimediów, gdy oba są aktywne).
• Dodano nową pozycję w Plik > Ostatnie pliki: „Wyczyść ostatnie pliki”, aby szybko wyczyścić listę ostatnio używanych dokumentów.
• Rozszerzono opcje bitrate w konwersji audio i ustawieniach nagrywania podcastu: dodano niższe wartości (64/96 kbps) oraz zwiększono MP3 do 320 kbps, z ujednoliconą walidacją i obsługą enkodera.
• Rozszerzono podział audiobooka według czasu do 60 minut.
• Ulepszono podział audiobooka na części: użytkownik może teraz ręcznie wpisać liczbę części, z walidacją od 1 do 100.
• Dodano nowy tryb Widok > Tylko do odczytu, aby blokować przypadkowe edycje tekstu przy zachowaniu pełnego odczytu i nawigacji po dokumentach.
• Dodano dostępny pasek postępu podczas aktualizacji programu, aby czytniki ekranu mogły na bieżąco śledzić postęp pobierania.
• Dodano nowy, dyskretny pasek stanu w głównym oknie z liczbą znaków, słów oraz pozycją wiersz/kolumna (np. "Znaki (ze spacjami): 11. | Słowa: 2. | Ln 1, Col 12"), bez zakłócania fokusu NVDA.
• Dodano nową opcję w menu Widok dla zawijania wierszy, aby można było szybko włączać/wyłączać zawijanie bez otwierania Opcji.
• Dodano w Edycja > Tekst nowe akcje zwiększania/zmniejszania wcięcia ze skrótami Ctrl+Shift+. (wcięcie) i Ctrl+Shift+, (usuń wcięcie), ponieważ gdy włączone jest „Pokaż głosy w edytorze”, klawisz Tab jest zarezerwowany do nawigacji w panelu głosów.
• Dodano lokalizowane wyświetlanie daty i godziny w artykułach RSS oraz odcinkach podcastów, z formatem dopasowanym do języka interfejsu.
• Dodano nową akcję w menu kontekstowym RSS do udostępniania wybranego artykułu przez e-mail.
• Dodano szczegółowe opcje potwierdzania usuwania w Opcje > RSS i podcasty: dla RSS (kanał/artykuł/oba/brak) i dla podcastów (podcast/odcinek/oba/brak).
• Dodano konfigurowalne szybkie kopiowanie RSS skrótem Ctrl+C (Opcje > RSS i podcasty): kopiowanie tytułu, URL, treści artykułu albo wszystkiego razem.
• Ujednolicono przepływ RSS: „Dodaj źródło” obsługuje teraz zarówno adresy URL feedów, jak i słowa kluczowe (z automatycznym generowaniem feedu Google News), bez osobnej funkcji wyszukiwania.
• Po naciśnięciu Ctrl+A program ogłasza teraz zakończenie akcji, co daje czytelniejszą informację zwrotną dla czytników ekranu.
• Dodano Shift+F3 dla „Znajdź poprzedni” w menu Edycja, jako uzupełnienie F3 „Znajdź następny”.
• Ulepszono komunikat po zamianie, dodając poprawne formy liczby pojedynczej i mnogiej (np. „1 zamianę” vs „2 zamiany”).
• Dodano w oknie słownika wybór języka wyszukiwania, z domyślną opcją Auto (język interfejsu) oraz możliwością ręcznego wyboru.
• Dodano nową kartę Skróty w Opcjach do personalizacji skrótów klawiaturowych, z wykrywaniem konfliktów i ostrzeżeniem, gdy skrót jest już przypisany do innej akcji.
• Dodano wstępną obsługę przełączników wiersza poleceń: `-h`/`--help` pokazują szybką pomoc, a `--version` wypisuje wersję programu.
• Uproszczono ręczną regulację prędkości i tonu: pola ręczne używają teraz skali wyśrodkowanej na 100, gdzie 100 oznacza wartość normalną.
• Ulepszono wybór głosów Microsoft zarówno w Opcje > Głos, jak i w panelu głosów edytora: dodano zlokalizowaną listę języków do filtrowania głosów po języku, a tryb „tylko głosy wielojęzyczne” pozostał pojedynczą listą bez podziału na języki (lista języków jest wtedy ukrywana).
• Dodano konfigurację głosu dialogów w Opcje > Głos z pełną nawigacją klawiszem Tab, opartą na tym samym modelu głosów co główny interfejs (silnik, filtr języka Edge, głos oraz etykietowane szybkość/ton/głośność); dodano też opcjonalny drugi głos dialogów z tymi samymi kontrolkami (silnik, filtr języka Edge, głos, szybkość/ton/głośność) do naprzemiennego czytania dialogów; reguły dialogów są zapisywane w konfiguracji `.ini`, bez modyfikowania tekstu dokumentu.
• Ulepszono etykietę Cofnij: pozycja Edycja > Cofnij pokazuje teraz, co zostanie cofnięte (np. edycja tekstu, cytowanie/odcytowanie linii lub wstawienie tagu głosu), pozostając niedostępna, gdy nie ma czego cofać.
Poprawki błędów
• Naprawiono obsługę otwierania RTF: pliki `.rtf` są teraz wyodrębniane i wyświetlane jako czytelny tekst, a nie surowy znacznik RTF (np. `{\\rtf1...}`).
• Naprawiono otwieranie chińskich plików tekstowych kodowanych jako GB18030/GBK: Sonarpad poprawnie je wykrywa i dekoduje, eliminując nieczytelny tekst (mojibake).
• Ulepszono tworzenie audiobooków M4B o metadane i znaczniki rozdziałów; naprawiono problem „chipmunk” (zbyt wysoki/szybki głos) w generowanych plikach M4B.
• Naprawiono interfejs bitrate w oknie zapisu audiobooka: usunięto teksty zakodowane na stałe po włosku i dodano 64 kbps do listy wybieralnych bitrate.
• Naprawiono „Zapisz wszystko” (Ctrl+Shift+S): wszystkie otwarte zmodyfikowane dokumenty są teraz wykrywane niezawodnie (także nowe/niezapisane karty), a zapis wszystkich działa poprawnie dla każdego pliku, otwierając „Zapisz jako”, gdy to potrzebne.
• Poprawiono kolejność artykułów RSS z Google News: gdy data publikacji jest dostępna, artykuły są teraz wyświetlane od najnowszego do najstarszego.
• Poprawiono powiązanie etykiet NVDA w oknie słownika: pole wyszukiwania i lista języka ogłaszają teraz właściwe etykiety.
• Poprawiono obsługę klawiatury w oknie Właściwości RSS/Podcast: Tab/Shift+Tab przechodzą teraz do przycisku OK, Enter aktywuje OK, Esc bezpiecznie zamyka okno, a fokus poprawnie wraca do listy RSS/Podcast.
• Poprawiono historię cofania w RSS/Podcast: Ctrl+Z obsługuje teraz wielopoziomowe cofanie usunięć (artykułów/odcinków i źródeł), a nie tylko ostatniej akcji.
• Ulepszono komunikaty usuwania w RSS/Podcast dzięki jawnym ogłoszeniom (usunięto RSS, usunięto artykuł RSS, usunięto odcinek podcastu).
• Ulepszono zachowanie fokusu po usunięciu/cofnięciu w RSS/Podcast: w RSS w razie potrzeby niezawodnie wybierany jest pierwszy kanał, a powtarzanie komunikatów czytnika ekranu podczas opóźnionej ponownej selekcji zostało ograniczone.

Wersja 0.6.6 – 2026-02-13
Ulepszenia
• Dodano „Automatyczne formatowanie dla TTS” w menu Edycja, aby szybko przygotować tekst do odczytu (usuwa markdown/cudzysłowy i scala połamane linie).
• Ulepszono wstawianie tagów głosu: gdy tekst jest zaznaczony, tagi są teraz poprawnie nakładane zarówno na pojedynczą linię, jak i na zaznaczenie wielowierszowe.
• Dodano opcję w ustawieniach Audio, aby wybrać domyślny folder zapisu audiobooków (domyślnie: Dokumenty\\Sonarpad Audiobooks).
• W oknie zapisu audiobooka, gdy aktywny jest podział na części, dodano nową opcję (włączoną domyślnie) tworzenia dedykowanego podfolderu dla wygenerowanych części.
• Eksport audiobooków zapisuje teraz MP3 w stereo z bitrate wybranym przez użytkownika dla głosów Edge, SAPI5 i SAPI4.
• Dodano obsługę głosów SAPI5 32-bit przez bridge, aby można było używać także głosów dostępnych tylko w silnikach 32-bit.
• Przeniesiono funkcje głosowe do dedykowanego menu „Głos i audio” oraz dodano/doprecyzowano opcję „Konwertuj audio”, która służy do konwersji dowolnego obsługiwanego pliku multimedialnego do MP3, AAC, OGG, Opus, FLAC, WAV i AIFF.
• Dodano usuwanie pojedynczych artykułów RSS i pojedynczych odcinków podcastów (klawisz Delete + menu kontekstowe z potwierdzeniem), bez usuwania całego źródła RSS/podcastu, wraz z cofnięciem ostatniego usunięcia (pojedynczy artykuł/odcinek lub całe źródło RSS/podcastu).
• Dodano eksport źródeł RSS do OPML w oknie RSS, aby łatwo zapisać i ponownie zaimportować aktualne źródła.
• Dodano funkcję „Wyszukaj RSS po słowie kluczowym” w oknie RSS: po wpisaniu słowa kluczowego Sonarpad automatycznie generuje adres RSS Google News i otwiera okno dodawania źródła z już uzupełnionymi polami, dzięki czemu feed tematyczny można utworzyć jednym krokiem.
• Dodano tłumaczenie serbskie dzięki Mila Kuran.
• Dodano tłumaczenie ukraińskie dzięki Ivan Shtefuriak.
• Dodano otwieranie wielu plików multimedialnych naraz: po otwarciu kilku plików tworzona jest kolejka odtwarzania zamiast zastępowania bieżącego pliku.
• Dodano skróty zmiennego przewijania podczas odtwarzania: przy bazie 1 minuty strzałka lewo/prawo przewija o 60 s, Shift+strzałka lewo/prawo o 20 s, a Ctrl+strzałka lewo/prawo o 3 minuty.
• Dodano skróty do poprzedniego/następnego utworu w odtwarzaczu: Ctrl+PageUp i Ctrl+PageDown.
• Dodano opcję „Reset głośności” i zgrupowano akcje resetu w dedykowanym podmenu „Reset” w Odtwarzaniu, razem z „Reset prędkości” i „Reset tonu”.
• Ulepszenia instalatora: setup.exe pozwala teraz wybrać między skojarzeniem wszystkich obsługiwanych typów plików a ręcznym wyborem rozszerzeń; MSI również udostępnia wybór per rozszerzenie w drzewie funkcji (domyślnie bez zmian: wszystko włączone).
• Dodano nowe menu „Okno” z opcją „Otwarte dokumenty...”, aby szybko przełączać się między aktualnie otwartymi plikami.
• Zaktualizowano opcję Widok > Czcionka: pełny selektor został zastąpiony szybkim podmenu z popularnymi czcionkami (Arial, Calibri, Consolas, Segoe UI, Tahoma, Verdana, Times New Roman, Georgia), z zachowaniem aktualnego rozmiaru tekstu.
• Ulepszono odczyt RSS i podcastów dzięki dwóm osobnym komunikatom: węzły źródeł ogłaszają „nowe elementy”, gdy feed/podcast ma nowości, a pojedyncze artykuły RSS i odcinki podcastów ogłaszają „nieprzeczytane”/„nieodtworzone”; to zachowanie można wyłączyć w Opcjach.
Poprawki błędów
• Naprawiono wyodrębnianie tekstu EPUB dla książek zawierających komentarze HTML inline (<!-- ... -->): tekst rozdziałów jest teraz poprawnie parsowany zamiast być częściowo lub całkowicie pomijany.
• Naprawiono słownik Wiktionary dla języka hiszpańskiego i obsługę cache słownika: słowa takie jak „agua” są teraz poprawnie znajdowane, a stare wpisy „nie znaleziono słowa” nie są już ponownie używane.
• Naprawiono kodowanie przy imporcie artykułów RSS dla niektórych hiszpańskich źródeł (np. El Mundo): akcenty i „ñ” są teraz poprawnie zachowywane w tymczasowym edytorze.
• Naprawiono dekodowanie ANSI dla plików środkowoeuropejskich (np. czeski/polski): Sonarpad lepiej rozróżnia teraz UTF-8 i ANSI oraz wybiera właściwą stronę kodową (w tym Windows-1250), dzięki czemu znaki diakrytyczne nie są zniekształcone.
• Naprawiono zapisywanie źródeł RSS z parametrami w URL (np. `rss.aspx?c=...`): takie feedy są teraz poprawnie zapisywane i przywracane po ponownym uruchomieniu Sonarpad.
• Naprawiono otwieranie plików wskaźnikowych Google Drive (`.gdoc`, `.gsheet`, `.gslides`) z menu kontekstowego Eksploratora: gdy bezpośredni odczyt kończył się błędem „Incorrect function (os error 1)”, Sonarpad używa teraz fallbacku shell-open i dokument otwiera się poprawnie.
• Naprawiono odczyt starszych plików Excel `.xls` (Excel 2010): stare pliki binarne są teraz poprawnie wykrywane i dekodowane zamiast pokazywać uszkodzony tekst (np. `ÐÏ_à¡±...`).
• Naprawiono sposób ogłaszania błędów pisowni: błędy są teraz ponownie odczytywane podczas późniejszego przeglądania tekstu, a ten sam błąd jest znów zgłaszany po usunięciu i ponownym wpisaniu.
• Naprawiono operacje tekstowe na liniach (np. Ctrl+Q / Ctrl+Shift+Q, sortowanie/odwracanie/unikalne/łączenie linii): po zaznaczeniu jednej linii przez Shift+Strzałka w dół sąsiednie linie nie są już łączone ani obcinane.
• Naprawiono działanie wielowierszowe operacji liniowych (Ctrl+Q / Ctrl+Shift+Q i narzędzia pokrewne): gdy RichEdit zwraca separatory linii jako samo CR, są one teraz poprawnie normalizowane i przetwarzane są wszystkie zaznaczone linie bez ucinania pierwszego znaku.
• Rozszerzono normalizację wejścia TTS o widoczne symbole spacji/tabulatora/nowej linii (␠/U+2420, ␣/U+2423, ␉/U+2409, ␊/U+240A, ␍/U+240D, ␤/U+2424), które przy głosach wielojęzycznych mogły powodować powtarzanie akapitów.
• Dopracowano sanitizację tekstu dla Edge TTS przez jedną, wspólną ścieżkę walidacji: normalizacja nietypowych/niewidocznych spacji, skracanie długich sekwencji interpunkcji (np. "...", "!!!", "???") oraz pomijanie fragmentów złożonych wyłącznie z interpunkcji, aby uniknąć pętli odtwarzania.
• Naprawiono komunikat czasu odtwarzania (Ctrl+I) dla strumieni MP3/podcast: bieżący czas jest teraz ograniczany do długości ścieżki, a odtwarzanie zatrzymuje się automatycznie, gdy pozycja wyjdzie poza koniec.
• Ulepszono pokrycie lokalizacji instalatora: setup.exe zawiera teraz także czeski, polski, francuski i serbski, a MSI pozostaje pojedynczym pakietem en-US, aby uniknąć zamieszania w wydaniach.
• Naprawiono czyszczenie podczas deinstalacji wpisów menu kontekstowego: „Otwórz za pomocą Sonarpad” jest teraz usuwane niezawodnie, także w starszych scenariuszach rejestru.
• Naprawiono niezawodność pauzy/wznawiania w SAPI5: pauza pod F4 działa teraz poprawnie, a wznowienie wraca do oczekiwanego miejsca zamiast startować od początku.
• Naprawiono przepływ pauza + przewijanie + wznowienie w odtwarzaniu multimediów: po pauzie i przewinięciu Strzałka lewo/prawo naciśnięcie Spacji niezawodnie wznawia od bieżącej pozycji zamiast zatrzymywać odtwarzanie lub uruchamiać je od początku.

Wersja 0.6.5 – 2026-02-07
Ulepszenia
• Poprawiona wersja hiszpańska dzięki Arturo Fernandez Rivas.
• Dodano opcję dzielenia audiobooków EPUB na rozdziały.
• Importy RSS używają teraz dedykowanej tymczasowej karty (zlokalizowany tytuł); Zapisz jako zamienia ją w zwykły dokument.
• Komunikaty czytnika ekranu są teraz również wysyłane do JAWS, gdy jest dostępny.
Poprawki błędów
• Odczyt od kursora (F5) teraz zaczyna się dokładnie w miejscu kursora. Wcześniej mógł startować kilka linii wyżej, bo offset kursora nie odpowiadał pozycjom CRLF/UTF-16.
• Naprawiono problem z odświeżaniem: przy pisaniu na zaznaczeniu wcześniejszy tekst mógł znikać do czasu przesunięcia zaznaczenia.
• Poprawiono parser rozdziałów EPUB: strony okładki lub tylko z obrazami nie generują już odczytu CSS (np. "padding") ani tytułów „Sconosciuto”.
• Naprawiono błąd przy dzieleniu audiobooków z EPUB według czasu: Edge TTS mógł się wyłożyć na pustych lub zbyt długich fragmentach ("Edge audio not sent").
• Artykuły RSS teraz dekodują encje HTML (np. &quot;, &amp;, &lt;, &gt;).
• Zapis/Zapisz jako teraz podpowiada nazwę istniejącego pliku przy zapisie formatów nienadpisywalnych (np. EPUB), zamiast pierwszej linii.
• Naprawiono problem, przez który podcasty z nowymi odcinkami nie były oznaczane jako nieodtworzone, oraz zmieniono „Nieodsłuchane” na „Nieodtworzone” jako bardziej profesjonalne.

Wersja 0.6.4 – 2026-02-05
Ulepszenia
• Program został przemianowany na Sonarpad, aby podkreślić dźwięk i audio jako klucz tego programu.
• Dodano wybór ścieżek audio w menu Odtwarzanie dla plików multimedialnych z wieloma ścieżkami audio (np. MKV z wieloma językami).
• Podcasty teraz wyraźnie wskazują te nieprzesłuchane z prefiksem „Nieprzesłuchane" przed nazwą.
• Nowy system tagów do zmiany głosu w tekście. Przykłady:
  - Głosy Microsoft (Edge): <voice edge it-IT-IsabellaNeural>Cześć</voice>
  - Głosy SAPI5: <voice sapi5 Microsoft Helena Desktop>Cześć</voice>
  - Głosy SAPI4: <voice sapi4 #1>Cześć</voice>
  - Z prędkością/tonem/głośnością: <voice edge it-IT-ElsaNeural speed=-20 pitch=-5 volume=-10>Cześć</voice>
• Rozszerzono kategorie podcastów.
• Dodano opcję w menu kontekstowym tworzenia audiobooka z zaznaczenia.
• Dodano podział audiobooka według długości, z możliwością wyboru nazwy pierwszego pliku.
• Zlokalizowano etykietę autora w odczycie artykułów (np. "by", "di", "par").
• Dodano opcje wcięć (tabulatory/spacje z szerokością) oraz Tab/Shift+Tab do wcinania/odwcinania zaznaczonych linii.
• Poprawiono czyszczenie Markdown: obsługa wypunktowań „*” gdy zachowanie list jest wyłączone.
Poprawki błędów
• Naprawiono błąd, przez który audiobooki SAPI4 mogły być tworzone inaczej niż oczekiwano.
• Okno Znajdź w plikach: naciśnięcie Enter na wyniku otwiera teraz w prawidłowej pozycji fragmentu, a Esc wraca do wyników.
• Okno Opcje: poprawiono układ wizualny kart Ogólne, Głos, Edytor i Audio, aby uniknąć brakujących lub uciętych kontrolek.
• Naprawiono problem z zakładkami podczas zmiany prędkości odtwarzania.
• Naprawiono problem z Podcast Index i kategoriami, które nie wyświetlały się poprawnie.
• Naprawiono problem z apostrofem rozbijającym czytanie: nie ma już osobnego czytania dialogów, używane są tagi głosu.

Wersja 0.6.3 – 30.01.2026
Ulepszenia
• Ulepszone wykrywanie mikrofonu.
• Dodano natychmiastowe odtwarzanie dla wszystkich formatów.
Poprawki błędów
• Naprawiono awarię w oknie kategorii podcastów.

Wersja 0.6.2 – 30.01.2026
Nowe funkcje
• Dodano obsługę wykonywania plików (Shift+F5). Użytkownicy mogą wybrać interpreter (np. python) w Opcjach, wyszukać go na komputerze, a naciśnięcie Shift+F5 uruchamia bieżący skrypt. Pliki HTML otwierają się w przeglądarce.
• Dodano obsługę plików wskaźników Google Docs (.gdoc, .gsheet, .gslides), które automatycznie otwierają się w domyślnej przeglądarce.
• Dodano obsługę formatu audiobooków M4B (Apple/AAC).
• Dodano opcję "Pokaż odcinki" w menu kontekstowym wyników wyszukiwania podcastów, aby przeglądać i odtwarzać odcinki bez subskrypcji.
• Dodano funkcję "Idź do linii" (menu Edycja lub Ctrl+J), aby szybko przejść do konkretnego numeru linii.
• Dodano opcje menu kontekstowego do porządkowania kanałów RSS i podcastów (alfabetycznie lub według daty).
• Dodano domyślne wietnamskie kanały RSS.
• Dodano pole testowe mikrofonu w oknie nagrywania, aby sprawdzić poziomy przed rozpoczęciem.
• Dodano "Pokaż opis" dla odcinków podcastów w menu kontekstowym.
• Dodano obsługę rozszerzonych formatów audio/wideo przez FFmpeg: mkv, avi, mov, m4v, webm, mpg, ts, wmv, flv, vob, 3gp, flac, ogg, wma, aiff.
• Dodano obsługę zsynchronizowanego odczytu napisów (srt, vtt, ass, sub, sbv, lrc, smi) za pomocą NVDA lub wybranego głosu. Program szuka pliku napisów o tej samej nazwie co plik multimedialny. Dodano opcje "Importuj napisy" i "Usuń napisy" w menu Odtwarzanie dla plików o różnych nazwach.
• Dodano skojarzenia plików dla wszystkich nowych obsługiwanych formatów audio/wideo w menu kontekstowym "Otwórz w Sonarpad".
• Dodano ustawienie regulacji wysokości dźwięku dla dowolnego pliku.
• Dodano opcję w Ustawieniach ogólnych, aby włączyć lub wyłączyć anonimowe raporty o błędach. Dodano wpis w menu Pomoc, aby utworzyć diagnostyczny plik ZIP.
• Dodano opcję używania innego głosu dla dialogów, zarówno do czytania na żywo, jak i tworzenia audiobooków.
• Dodano przeglądarkę kategorii podcastów do eksplorowania podcastów według kategorii (biznes, sztuka, sport itp.).
Ulepszenia
• Otwarcie pliku audio/wideo z Eksploratora otwiera teraz bezpośrednio widok odtwarzacza zamiast edytora tekstu.
• Usunięto monit o OCR dla niedostępnych plików PDF; OCR jest teraz wykonywany automatycznie, aby poprawić szybkość i wrażenia użytkownika.
• Ulepszono Dostępny Terminal: odczyt NVDA pamięta teraz ostatnio przeczytaną linię dla lepszej ciągłości.
• SAPI 4: Tworzenie audiobooków jest teraz w pełni zrównoleglone i prawie natychmiastowe. Dodano monit o wybór liczby współbieżnych procesów.
• SAPI 4: Wyeliminowano wąskie gardło konwersji WAV do MP3 poprzez równoległe konwertowanie fragmentów podczas syntezy.
• SAPI 4: Ulepszono obsługę błędów i automatyczne czyszczenie plików tymczasowych.
• Okno Znajdź: Zmieniono nazwę "Regex" na "Wyrażenie regularne" dla jasności i dodano brakujące tłumaczenia dla opcji wyszukiwania.
• Audiobooki M4B: Lepsza obsługa wyjścia; podział na części/znaczniki tworzy teraz pojedynczy plik M4B z metadanymi rozdziałów, w tym tytułem i autorem.
• Odtwarzacz: Naprawiono precyzję zakładek i ogłaszania czasu, gdy prędkość odtwarzania nie wynosi 1.0x.
• Przywrócono nawigację Ctrl+Tab i Ctrl+Shift+Tab w Opcjach.
• Dodano opcję w menu Odtwarzanie, aby natychmiast zresetować prędkość do Normalnej (1.0x).
• Zaktualizowano wszystkie zależności do najnowszych wersji dla lepszej wydajności i stabilności.
• Zintegrowano FFmpeg z dynamicznym ładowaniem DLL, aby zapewnić kompatybilność bez blokowania uruchamiania.
• Zaktualizowano filtry pobierania podcastów, aby uwzględnić nowe formaty audio/wideo.
• Zablokowano zapisywanie plików audio/wideo przez Ctrl+S, aby uniknąć uszkodzenia.
• Ulepszono import transkrypcji YouTube, czyniąc go bardziej odpornym i niezawodnym.
• Ulepszono odporność dzielenia audiobooków na części, zapewniając, że żaden tekst nie zostanie utracony.
• Instalator jest teraz w pełni wielojęzyczny, obsługując włoski, angielski, hiszpański, portugalski, szwedzki i wietnamski w oparciu o język systemu użytkownika. Angielski jest domyślny dla nieobsługiwanych systemów.
• Kategorie podcastów: naciśnięcie Enter na kategorii potwierdza teraz wybór (odpowiednik przycisku OK).
• Ulepszono system wykrywania zawieszania, aby uniknąć fałszywych alarmów, gdy otwarte są modalne okna dialogowe (komunikaty o błędach, "nie znaleziono tekstu").
Poprawki
• Naprawiono błąd, przez który dziennik zmian nie otwierał się przy starcie.
• Naprawiono błąd, przez który monit o OCR nie pojawiał się dla niedostępnych plików PDF otwieranych z Eksploratora.
• Naprawiono błąd przy starcie, który mógł powodować utratę fokusu lub zamknięcie okna natychmiast po otwarciu.
• Naprawiono krytyczny błąd w wyszukiwaniu regex, który uniemożliwiał znalezienie tekstu, w tym problemy z "Wyszukiwaniem cyklicznym" i opcją "Kropka odpowiada nowej linii" z zakończeniami linii Windows.
Lokalizacja
• Dodano tłumaczenie na język polski.
• Dodano tłumaczenie na język francuski.
• Dodano tłumaczenie na język czeski (dzięki Radkowi Žaludowi i Jiri Holzingerowi).

Wersja 0.6.1 – 20.01.2026
Poprawki
• Naprawiono błąd, przez który włączenie "Pokaż głosy w edytorze" powodowało zatrzymanie odtwarzania podcastu.
• Naprawiono problem, przez który niektórych podcastów nie można było dodać przez URL, ponieważ URL był ucinany.
• Naprawiono błąd, przez który normalne adresy URL nie mogły być już dodawane w funkcji kanału RSS.
• Naprawiono problem, przez który opcja języka Wikipedii była wyświetlana wielokrotnie w różnych zakładkach ustawień.
• Usunięto tworzenie plików debugowania, które były nieprawidłowo generowane nawet w trybie release.
Ulepszenia
• Ulepszono obsługę głosów Microsoft, które teraz używają dedykowanej metody odtwarzania z innym user agentem.
• Dodano obsługę plików MP4.

Wersja 0.6.0 – 20.01.2026
Nowe funkcje
• Dodano sprawdzanie pisowni. Z menu kontekstowego użytkownicy mogą sprawdzić, czy bieżące słowo jest poprawne, a jeśli nie, uzyskać sugestie pisowni.
• Dodano import i eksport podcastów przez pliki OPML.
• Dodano obsługę wyszukiwania Podcast Index oprócz iTunes. Użytkownicy mogą wprowadzić swój darmowy klucz API i sekret (generowane tylko przy użyciu adresu e-mail).
• Dodano obsługę głosów SAPI4, zarówno do czytania w czasie rzeczywistym, jak i tworzenia audiobooków.
• Dodano automatyczny fallback OCR dla niedostępnych plików PDF: gdy nie znaleziono tekstu do wyodrębnienia, dokument jest rozpoznawany przez OCR.
• Dodano obsługę słownika przy użyciu Wiktionary. Naciśnięcie klawisza Aplikacji pokazuje definicje, a gdy dostępne, synonimy i tłumaczenia na inne języki.
• Dodano import artykułów z Wikipedii z wyszukiwaniem, wyborem wyników i bezpośrednim importem do edytora.
• Dodano skrót Shift+Enter w module RSS, aby otworzyć artykuł bezpośrednio na oryginalnej stronie internetowej.
Ulepszenia
• Wybór mikrofonu jest teraz zawsze respektowany przez aplikację.
• W oknie podcastów naciśnięcie Enter na odcinku teraz natychmiast ogłasza "ładowanie" przez NVDA, aby potwierdzić akcję.
• W wynikach wyszukiwania podcastów naciśnięcie Enter teraz subskrybuje wybrany podcast.
• Poprawiono i ulepszono etykiety dla skrótów Ctrl+Shift+O i Podcast Ctrl+Shift+P.
• Prędkość i głośność odtwarzania są teraz zapisywane w ustawieniach i zachowywane dla wszystkich plików audio.
• Dodano dedykowany folder pamięci podręcznej dla odcinków podcastów. Użytkownicy mogą zachować odcinki przez "Zachowaj podcast" w menu Odtwarzanie. Pamięć podręczna jest automatycznie czyszczona po przekroczeniu rozmiaru zdefiniowanego przez użytkownika (Opcje → Audio).
• Znacznie ulepszono pobieranie artykułów RSS przy użyciu podszywania się pod libcurl z profilami Chrome i iPhone, zapewniając kompatybilność z ~99% stron.
• Dodano stan przeczytany / nieprzeczytany dla artykułów RSS, z wyraźnym wskazaniem na liście RSS.
• Zamień wszystko teraz raportuje liczbę wykonanych zamian.
• Dodano przycisk Usuń podcast podczas nawigacji po bibliotece podcastów za pomocą Tab.
Poprawki
• Usunięto zbędny wpis "oczekująca aktualizacja" z menu Pomoc (aktualizacje są już obsługiwane automatycznie).
• Naprawiono błąd, przez który naciśnięcie Ctrl+S na otwartym pliku MP3 zapisywało i uszkadzało plik.
• Naprawiono problem z interfejsem użytkownika, gdzie "Audiobooki wsadowo" były wyświetlane jako "(B)… Ctrl+Shift+B" (usunięto zbędną etykietę).
• Naprawiono inteligentne cudzysłowy: po włączeniu normalne cudzysłowy są teraz poprawnie zastępowane inteligentnymi cudzysłowami.
• Naprawiono błąd, przez który użycie "Idź do zakładki" resetowało prędkość odtwarzania do 1.0.
• Naprawiono problem, przez który już pobrane odcinki podcastów były pobierane ponownie zamiast używać wersji z pamięci podręcznej.
Skróty klawiszowe
• F1 otwiera teraz Przewodnik pomocy.
• F2 sprawdza teraz dostępność aktualizacji.
• F7 / F8 skaczą teraz do poprzedniego lub następnego błędu pisowni.
• F9 / F10 szybko przełączają między ulubionymi głosami.
Ulepszenia deweloperskie
• Błędy nie są już milcząco pomijane: usunięto wszystkie wzorce let _ =, a błędy są teraz jawnie obsługiwane (propagowane, logowane lub obsługiwane z fallbackami w stosownych przypadkach).
• Projekt teraz nie kompiluje się, jeśli są ostrzeżenia: zarówno cargo check, jak i cargo clippy muszą przejść czysto, z zaostrzonymi lintami i usuniętymi allow tam, gdzie to możliwe.
• Usunięto niestandardowe implementacje, takie jak pomocniki w stylu strlen / wcslen. Długości ciągów i buforów UTF-16 są teraz wyznaczane z danych posiadanych przez Rust zamiast skanowania pamięci.
• Obsługa DLL została uporządkowana i skonsolidowana wokół libloading, unikając niestandardowej logiki ładowania i parsowania PE.
• Usunięto ręcznie pisane pomocniki parsowania bajtów; całe parsowanie bajtów używa teraz standardowych from_le_bytes / from_be_bytes na sprawdzonych wycinkach.
Te zmiany zmniejszają niepotrzebne użycie unsafe, eliminują potencjalne niezdefiniowane zachowanie i sprawiają, że baza kodu jest bardziej idiomatyczna, solidna i łatwa w utrzymaniu.

Wersja 0.5.9 - 13.01.2026
Nowe funkcje
• Dodano zmianę kolejności RSS z menu kontekstowego (góra/dół/do pozycji) ze sprawdzaniem nieprawidłowej pozycji.
• Dodano menu kontekstowe artykułu z otwieraniem oryginalnej strony i udostępnianiem przez WhatsApp, Facebook i X.
• Dodano skrót Esc, aby powrócić z importowanych artykułów do listy RSS.
• Dodano tryb podcastów: wyszukiwanie, subskrypcja, słuchanie; zmiana kolejności subskrypcji; Esc zatrzymuje odtwarzanie i wraca do listy; Enter na odcinku rozpoczyna odtwarzanie.
• Dodano kontrolę prędkości odtwarzania dla podcastów i plików MP3.
• Dodano Ctrl+T, aby przejść do określonego czasu.
• Dodano przycisk podglądu głosu za polem wyboru głośności.
• Dodano wyszukiwanie i zamianę za pomocą wyrażeń regularnych (styl Notepad++).
• Dodano import RSS z plików OPML i TXT.
• Dodano opcję włączenia "Otwórz w Sonarpad" w Eksploratorze plików, w tym w wersjach portable.
Ulepszenia
• Ulepszono wybór prędkości/tonu/głośności głosu, respektując maksymalne limity TTS.
• Różne ulepszenia RSS, aby pobierać wszystkie artykuły bez przesuwania fokusu NVDA podczas aktualizacji.
• Ulepszono odtwarzanie dźwięku dzięki dedykowanemu menu, ogłaszaniu czasu Ctrl+I i głośności do 300%.
• Dodano brakujące skróty dla niektórych funkcji.
• Zreorganizowano menu Edycja z podmenu czyszczenia tekstu.
• Zreorganizowano Opcje w zakładki, z nawigacją Ctrl+Tab i Ctrl+Shift+Tab.
• Czytnik RSS pobiera teraz pełną treść artykułu, pasującą do widoku przeglądarki.
Poprawki
• Naprawiono usuwanie cyfr na początku linii podczas czyszczenia Markdown.
• Naprawiono wyzwalanie cofania przez AltGr+Z.
• Naprawiono anulowanie nagrywania audiobooka, aby zatrzymywało się szybko.
Lokalizacja
• Dodano tłumaczenie wietnamskie (dzięki Anh Đức Nguyễn).

Wersja 0.5.8 - 10.01.2026
Nowe funkcje
• Dodano regulację głośności dla mikrofonu i dźwięku systemowego podczas nagrywania podcastów.
• Dodano nową funkcję importu artykułów ze stron internetowych lub kanałów RSS, w tym najważniejsze kanały dla każdego języka.
• Dodano funkcję usuwania wszystkich zakładek dla bieżącego pliku.
• Dodano funkcję usuwania zduplikowanych linii i zduplikowanych kolejnych linii.
• Dodano funkcję zamykania wszystkich kart lub okien z wyjątkiem bieżącego.
• Dodano wpis Darowizny w menu Pomoc dla wszystkich języków.
Ulepszenia
• Ulepszono dostępny terminal, aby zapobiec niektórym awariom.
• Ulepszono i naprawiono klawisze dostępu i skróty klawiaturowe w całej aplikacji.
• Naprawiono problem, przez który zamknięcie okna odtwarzania audio nie zatrzymywało odtwarzania.
• Dodano okna dialogowe potwierdzenia dla ważnych działań (np. usuń zduplikowane linie, usuń łączniki na końcu linii, usuń wszystkie zakładki w bieżącym pliku). Okno dialogowe nie jest wyświetlane, gdy akcja nie ma zastosowania.
• Dodano możliwość usuwania kanałów/stron RSS z biblioteki poprzez wybranie ich i naciśnięcie Delete.
• Dodano menu kontekstowe w oknie RSS do edycji lub usuwania kanałów/stron RSS.
• Usunięto ustawienie przenoszenia ustawień do bieżącego folderu; aplikacja teraz obsługuje to automatycznie na podstawie lokalizacji (jeśli folder exe nazywa się "sonarpad portable" lub exe znajduje się na dysku wymiennym, ustawienia trafiają do folderu exe w `config`, w przeciwnym razie `%APPDATA%\Sonarpad`, z fallbackiem do exe `config`, jeśli preferowany folder nie jest zapisywalny).

3. Dodano opcjonalną usługę Sonarpad AI do audiodeskrypcji z AI. Użytkownicy mogą nadal korzystać z własnego klucza API Gemini albo wybrać usługę Sonarpad i wpisać osobisty kod Sonarpad. Kod jest chroniony przez Windows DPAPI. W trybie usługi przygotowane fragmenty wideo są wysyłane bezpośrednio z komputera użytkownika do Google przez tymczasowy adres URL przesyłania Google; sonarpad.com nie otrzymuje ani nie przechowuje danych wideo. Backend jedynie autoryzuje dostęp, wymusza model Gemini/poziom Standard i limity użycia, rozlicza wykorzystanie oraz zwraca odpowiedź Gemini.

Version 0.5.7 - 2026-01-05
New features
• Added Batch Audiobooks feature to convert multiple files/folders at once.
• Added support for Markdown files (.md).
• Added file encoding selection when opening text files.
• Added option in the accessible terminal to announce new lines with NVDA.
Improvements
• Audiobook recording now saves natively to MP3 when selected.
• User can now choose the position of the "unsaved changes" asterisk (*) in the window title.
• Improved the update system robustness across different scenarios.
• Added "Remove Hyphens" in Edit menu to fix OCR line-endings.

Version 0.5.6 - 2026-01-04
Fixes
  Improved Find in Files so pressing Enter opens the file exactly at the selected snippet.
Improvements
  Added PPT/PPTX support (open as text).
  Opening non-text formats now saves as .txt to avoid formatting corruption (PDF/DOC/DOCX/EPUB/HTML/PPT/PPTX).
  Added podcast recording from microphone and system audio (File menu, Ctrl+Shift+R).

Version 0.5.5 – 2026-01-03
New features
• Added an accessible terminal optimized for large output and screen readers (Ctrl+Shift+P).
• Added a setting to save user settings in the current folder (portable mode).
Fixes
• Improved Find in Files snippets so the preview stays aligned with the match.

Version 0.5.4 – 2026-01-03
Improvements
• Fixed Normalize Whitespace (Ctrl+Shift+Enter).
• Added HTML/HTM support (open as text).

Version 0.5.3 – 2026-01-02
New features
• Added Find in Files.
• Added new text tools: Normalize Whitespace, Hard Line Break, and Strip Markdown.
• Added Text Statistics (Alt+Y).
• Added new list commands in the Edit menu:
• Order Items (Alt+Shift+O)
• Keep Unique Items (Alt+Shift+K)
• Reverse Items (Alt+Shift+Z)
• Added Quote / Unquote Lines (Ctrl+Q / Ctrl+Shift+Q).
Localization
• Added Spanish localization.
• Added Portuguese localization.
Improvements
• When an EPUB file is open, Save now automatically switches to Save As and exports the content as a .txt file to prevent EPUB corruption.

## 0.5.2 - 2026-01-01
- Added a changelog.
- Added open-with-Sonarpad options and file associations for supported files during installation.
- Improved message localization (errors, dialogs, audiobook export).
- Added part selection when using "Split audiobook based on text", with a "Require the marker at line start" option.
- Added YouTube transcript import with language selection, timestamp option, and improved focus handling.

## 0.5.1 - 2025-12-31
- Automatic updates with confirmation, improved error handling and notifications.
- Audiobook export improvements (text-based split, SAPI5/Media Foundation, advanced controls).
- TTS improvements (pause/resume, replacement dictionary, favorites).
- View menu and voice/favorites panels, text color and size.
- Default language from system locale and localization improvements.
- CI and Windows packaging (artifacts, MSI/NSIS, cache).

## 0.5.0 - 2025-12-27
- Modular refactor (editor, file handler, menu, search).
- Windows build/packaging workflow and README/license updates.
- Fix TAB navigation in the Help window.

## 0.5 - 2025-12-27
- Preliminary version bump.

## 0.1.0 - 2025-12-25
- Initial release: project structure and README.

10. Fixed segment reanalysis timing when mapping an independently analyzed mini-film back to the original movie. Sonarpad now applies the same small chunk-duration reconciliation used by the normal full analysis to descriptions, visual-evidence timestamps, and protected dialogue intervals, preventing progressive sub-second drift near the end of a chunk from moving narration onto speech.


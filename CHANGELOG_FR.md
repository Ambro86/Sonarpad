# Journal des modifications

Version 0.9.6 – 2026-09-09

1. Correction des blocages de certaines voix SAPI4 lorsque le suivi du curseur est activé. Le bridge attend désormais la fin réelle de la synthèse, sans désactiver le suivi.

2. Correction de la synchronisation entre microphone et audio système et des clics dus aux arrondis des horodatages pendant les enregistrements de podcasts. Ces corrections concernent aussi les sources enregistrées séparément.

3. La synthèse SAPI5 des audiodescriptions utilise désormais des processus séparés. En cas d’échec, elle reprend avec moins de processus simultanés, conserve les descriptions réussies et mémorise la limite fonctionnelle pour la voix.

Version 0.9.5 – 2026-09-07

Audiodescription avec IA
1. Amélioration de la robustesse lorsque Gemini renvoie temporairement `MALFORMED_RESPONSE` sans contenu exploitable. Sonarpad réessaie désormais exactement le même segment jusqu’à trois fois, avec une courte attente entre les tentatives, au lieu d’interrompre immédiatement toute l’audiodescription. Les réponses normales de Gemini, les blocages de sécurité, les erreurs de limite de jetons et le comportement existant des points de contrôle restent inchangés.

2. Correction d’un autre cas rare de `invalid Gemini chunk timeline` avec certaines vidéos dont les segments préparés par FFmpeg indiquent un petit écart cumulatif de durée. Sonarpad conserve le fonctionnement normal inchangé lorsque la chronologie est valide et n’utilise un réajustement prudent que si la chronologie normale échouerait et que l’écart total reste faible ; les écarts importants ou suspects continuent de produire une erreur.

3. Ajout du service optionnel Sonarpad AI pour les audiodescriptions avec IA. Les utilisateurs peuvent continuer à utiliser leur propre clé API Gemini ou choisir le service Sonarpad et saisir un code Sonarpad personnel. Le code est protégé avec Windows DPAPI. En mode service, les segments vidéo préparés sont envoyés directement du PC de l’utilisateur vers Google via une URL temporaire de téléversement Google ; sonarpad.com ne reçoit ni ne conserve les octets vidéo. Le backend autorise uniquement l’accès, impose le modèle Gemini/le niveau Standard et les limites d’utilisation, comptabilise la consommation et renvoie la réponse de Gemini.

4. Amélioration de la modification des projets d’audiodescription enregistrés. Il est maintenant possible de modifier plusieurs descriptions à la suite sans devoir appliquer chaque modification immédiatement. Les changements restent en mémoire ; avec « Appliquer le texte », Sonarpad vérifie chaque description modifiée par rapport à son intervalle enregistré puis enregistre ensemble toutes les modifications valides. Si une description ne tient pas dans son intervalle, le focus revient sur celle-ci sans perdre les autres modifications en attente.

5. Ajout de « Régler la voix » juste avant « Créer l’audiodescription ». La nouvelle fenêtre permet de choisir le moteur, la voix, la vitesse et le volume et comprend « Tester la voix ». Après OK, le focus revient exactement sur « Régler la voix » et les valeurs choisies sont appliquées à l’audiodescription à créer. Si cette fenêtre n’est pas utilisée, la synthèse reste exactement comme auparavant.


6. Les réglages de voix ont été étendus dans les projets d’audiodescription enregistrés. Dans Modifier le projet, en plus du moteur et de la voix, la vitesse, le volume et « Tester la voix » sont maintenant disponibles. Lors de l’application des paramètres vocaux, Sonarpad vérifie de nouveau toutes les descriptions existantes avec les nouveaux paramètres de synthèse et ne reconstruit le MP3 que si elles tiennent toujours dans leurs intervalles enregistrés. Les intervalles sans dialogue, les horodatages Visual Evidence, les descriptions Gemini et l’analyse temporelle du projet sont réutilisés sans relancer l’analyse IA.

Version 0.9.4 – 2026-09-04

Audiodescription avec IA
1. Correction d’un problème avec les vidéos Matroska/WebM contenant un horodatage de début interne anormalement élevé, qui pouvait interrompre la génération de l’audiodescription avec l’erreur « invalid Gemini chunk timeline ». Sonarpad normalise désormais la durée du fichier source et des segments Gemini uniquement lorsque ce schéma d’horodatage anormal est détecté, sans modifier les vidéos normales ni le comportement de l’audiodescription qui fonctionnait déjà.
2. Amélioration du ducking de l’audiodescription pour obtenir un mixage plus naturel. Le son original baisse désormais progressivement avant le début de la voix et revient plus doucement à son niveau normal après la description ; lorsque deux descriptions sont très rapprochées, le niveau de fond reste stable afin d’éviter des baisses et remontées répétées.
3. Correction d’un problème pouvant faire échouer l’exportation finale de l’audiodescription avec certains fichiers AVI et d’autres sources en audio 5.1/multicanal lorsque le décodage se terminait par une trame audio entrelacée incomplète. Sonarpad complète désormais en toute sécurité uniquement cette dernière trame partielle avec le nombre strictement nécessaire d’échantillons silencieux ; les trames déjà complètes et les fichiers mono, stéréo ou multicanaux normaux restent inchangés.


Version 0.9.3 – 2026-09-03

Voix SAPI5
1. Correction d’un problème qui pouvait empêcher certaines voix SAPI5 locales de parler lorsque le déplacement du curseur était activé, pendant la lecture à plusieurs voix ou lors de la création de livres audio/MP3. Sonarpad utilise désormais une méthode de synthèse SAPI5 vers un fichier fiable sous Windows et qui peut toujours être annulée, tandis que la lecture directe normale reste inchangée.
2. Correction de la position du curseur pendant la lecture des dialogues avec plusieurs voix. Les balises de voix ajoutées automatiquement par Sonarpad pour les dialogues sont désormais traitées uniquement comme des métadonnées de lecture et non comme des caractères présents dans l’éditeur, ce qui évite que le curseur avance trop loin après F4 ou F6. La lecture à une seule voix et les balises <voice> écrites explicitement dans les documents restent inchangées.

Audiodescription avec IA
1. Ajout de la case « Afficher la clé API » juste après le champ de la clé API Gemini. Elle est désactivée par défaut ; lorsqu’elle est activée, elle affiche temporairement la clé complète afin de vérifier qu’elle a été collée entièrement. À la réouverture de la fenêtre, la clé est de nouveau masquée.

Podcasts et Wikipédia
1. Après l’enregistrement d’un podcast, Sonarpad demande désormais s’il faut ouvrir le dossier contenant le fichier enregistré, comme c’est déjà le cas après l’enregistrement de médias YouTube/streaming.
2. Lorsqu’un autre article Wikipédia est importé dans un éditeur qui contient déjà du texte, le nouvel article est désormais ajouté à la fin au lieu d’être inséré en haut. Le curseur est placé au début du nouvel article importé.


Version 0.9.2 – 2026-09-02

Audiodescription par IA
1. Correction d’un problème qui pouvait faire échouer l’audiodescription par IA lors de l’export final en MP3 avec des vidéos contenant un son multicanal, par exemple en 5.1. Sonarpad convertit désormais automatiquement le son multicanal en stéréo uniquement lorsque cela est nécessaire pour l’encodage MP3, sans modifier les exports mono ou stéréo.
2. Lors du démarrage de la création d’une audiodescription avec IA pour une vidéo contenant plusieurs pistes audio, Sonarpad demande désormais quelle piste utiliser avant le traitement. La liste déroulante accessible se parcourt avec les flèches ; OK lance l’audiodescription avec la piste choisie, tandis qu’Annuler ferme la fenêtre d’audiodescription et rend le focus à l’éditeur Sonarpad.

YouTube et streaming
1. Correction d’un problème où le lancement de Créer une audiodescription par IA depuis une vidéo située à la page 2 ou suivante d’une playlist ou d’une chaîne YouTube pouvait rouvrir la fenêtre de sélection YouTube et retirer le focus à la fenêtre d’audiodescription. Sonarpad ferme désormais correctement le sélecteur sans revenir aux pages précédentes.
Version 0.9.1 – 2026-09-01

Téléchargements YouTube
• Correction d’un problème où les fenêtres de progression des téléchargements YouTube/streaming pouvaient revenir plusieurs fois au premier plan après un passage vers une autre application avec Alt+Tab. Les téléchargements continuent maintenant en arrière-plan sans voler le focus.
• Amélioration de l’accessibilité de la progression des téléchargements. En revenant dans la fenêtre de progression, les lecteurs d’écran peuvent lire l’état actuel et le pourcentage. Pour les playlists, Sonarpad indique aussi le numéro de l’élément en cours, le nombre total d’éléments et le titre.
• Correction de faux signalements de blocage du watchdog pendant de longs téléchargements et conversions alors que la fenêtre de progression restait réactive.
• Ajout d’une liste déroulante Format pour le téléchargement des playlists. Depuis la liste des vidéos, appuyez sur Tab pour choisir MP4, MP3, M4A, OPUS, OGG, WAV ou FLAC avant de lancer le téléchargement multiple.
• Réorganisation de l’enregistrement des médias en streaming. Le format et la qualité sont maintenant choisis au moment de l’enregistrement, et non dans la fenêtre initiale de recherche de streaming. « Enregistrer le média » ouvre une seule boîte de dialogue Format/Qualité et les téléchargements de playlists proposent les deux listes déroulantes.

Audiodescription avec IA
• Correction d’un problème qui pouvait empêcher le démarrage de l’audiodescription avec IA avec certaines vidéos MKV. Sonarpad gère désormais plus fiablement les vidéos dont les horodatages sont irréguliers ou manquants.

Version 0.9.0 – 2026-08-31

Audiodescription avec IA — nouvelle fonction principale
• Ajout de « Créer une audiodescription avec IA » dans Outils > Multimédia. Sonarpad analyse l’audio pour repérer les espaces sans dialogue, génère les descriptions avec Gemini et utilise les moteurs vocaux déjà disponibles, sans parler par-dessus les dialogues.
• Amélioration de la synchronisation entre les événements de la vidéo et les descriptions, avec des contrôles automatiques des temps générés par Gemini.
• « Activer les pauses étendues » est désactivé par défaut. Cette option peut être activée pour les contenus très dialogués ou offrant peu d’espace afin de permettre l’insertion de descriptions plus longues.
• Sonarpad peut essayer de reconnaître les personnages et d’utiliser leurs noms. Les catalogues de personnages peuvent être conservés entre les épisodes d’une série afin d’améliorer la continuité.
• Il est possible d’enregistrer le projet, de modifier les descriptions plus tard et de réexporter sans tout régénérer avec Gemini.
• Si le processus est interrompu, Sonarpad conserve la progression et permet de reprendre l’audiodescription. Si le quota Gemini est épuisé, il est possible d’attendre, de changer de modèle ou d’arrêter sans perdre le travail déjà terminé.
• La fenêtre permet de choisir la langue, le niveau de détail, le modèle Gemini, le moteur et la voix, et mémorise les préférences utilisées.
• Le module est disponible dans les 17 langues de Sonarpad. Pendant la génération, l’interface n’affiche que la progression, l’état courant et Annuler ; à la fin, le MP3 peut être ouvert directement dans le lecteur interne.

Livres numériques et documents
• Ajout de l’importation Kindle sans DRM aux formats MOBI, AZW et AZW3, avec texte et chapitres disponibles dans l’éditeur et l’index.
• Ajout de la prise en charge de DAISY 2.02 et DAISY 3. Les livres audio DAISY utilisent le lecteur interne de Sonarpad et respectent la navigation et les limites des chapitres.
• Kindle et DAISY sont importés sans écraser le fichier d’origine ; les Kindle protégés par DRM sont explicitement refusés.
• Correction de « Enregistrer sous » pour les EPUB : lorsqu’un format TXT ou autre est choisi, l’extension sélectionnée est utilisée et l’EPUB original reste associé au document ouvert.

RSS et articles
• Ajout de la sélection multiple des articles RSS pour en supprimer plusieurs en une seule opération.
• RSS prend désormais en charge de véritables dossiers conservés lors de l’importation et de l’exportation OPML, y compris les dossiers vides.
• Les flux peuvent être réordonnés dans le dossier courant avec Déplacer vers le haut, Déplacer vers le bas, Déplacer en haut, Déplacer en bas et Déplacer à la position.

Accessibilité, guides et interface
• Les guides Sonarpad ont été réorganisés avec un index et un guide complet de l’Audiodescription avec IA a été ajouté.
• Correction d’un problème de traduction allemande qui pouvait empêcher l’affichage d’Ouvrir, Enregistrer sous et d’autres fenêtres de sélection de fichiers.

Voix et langues
• Le catalogue téléchargeable Google TTS passe de 104 à 156 paquets et de 53 à 81 variantes linguistiques.
• Ajout de nouveaux paquets Google TTS et de noms localisés pour davantage de langues dans toute l’interface.

Version 0.8.4 – 2026-07-24

Modification des documents EPUB
• Sonarpad peut désormais non seulement ouvrir les documents EPUB, mais aussi les modifier et les enregistrer de nouveau au format EPUB tout en conservant la mise en forme d’origine, la table des matières, les notes de bas de page, les images, les feuilles de style, les métadonnées et les liens internes.
• Le format EPUB est disponible dans « Enregistrer sous » pour les documents ouverts à partir d’un EPUB. L’enregistrement ne met à jour que le texte modifié et conserve intacte la structure du livre.

Fiabilité des livres audio
• Correction d’un problème intermittent : après cinq échecs de Google TTS, une unité de synthèse pouvait être supprimée silencieusement et une partie du texte pouvait manquer dans le livre audio final.
• Les unités Google sont désormais réessayées jusqu’à leur réussite ou jusqu’à l’annulation par l’utilisateur. Le démarrage des processus est décalé afin de réduire les conflits temporaires avec Chrome et les fichiers ; Sonarpad interrompt également la création au lieu d’enregistrer un livre audio auquel il manque un segment.
• Les livres audio Edge réessaient désormais sans limite fixe les erreurs temporaires de réseau, WebSocket, délai d’attente, limitation du service et audio non valide, jusqu’à la réussite ou l’annulation par l’utilisateur, y compris avec des voix mixtes et le découpage par durée. SAPI4 et SAPI5 conservent des tentatives adaptatives mais limitées ; si un segment échoue toujours, Sonarpad arrête l’opération sans enregistrer de livre audio incomplet.

Navigation dans les bibliothèques numériques
• Les résultats de LibriVox, Internet Archive et Project Gutenberg utilisent désormais une navigation par pages comme YouTube : « Aller aux résultats précédents » apparaît au début de la liste et « Aller aux résultats suivants » à la fin.
• Les transitions de focus dans LibriVox ont été corrigées : lors de l’ouverture d’un livre ou d’un chapitre, le focus NVDA ne passe plus dans l’éditeur principal avant l’ouverture de la liste suivante ou du lecteur.
• Une protection du focus a été ajoutée pendant les recherches et le chargement des livres LibriVox : une fenêtre de chargement localisée reste au premier plan pendant toute la requête, empêchant le focus NVDA de passer à l’invite de commandes, à Windows Terminal ou à une autre application.

Téléchargement des playlists YouTube
• Ajout aux playlists YouTube d’une commande accessible de sélection multiple permettant de choisir les vidéos à télécharger sans modifier la commande « Enregistrer le média » de l’élément en cours de lecture.
• Les éléments sélectionnés sont téléchargés un par un avec le format et la qualité choisis à l’ouverture de la playlist, reçoivent des noms numérotés respectant l’ordre d’origine et sont enregistrés dans un dossier dédié à l’intérieur du dossier Médias configuré.
• La fenêtre propose « Tout sélectionner » et « Tout désélectionner », annonce le nombre d’éléments sélectionnés, permet d’annuler en conservant les fichiers déjà terminés et signale clairement les éléments qui n’ont pas pu être téléchargés.
• Les éléments de la playlist sont désormais de véritables cases à cocher natives : les lecteurs d’écran annoncent automatiquement le titre, le type de contrôle et l’état coché ou non coché, sans ajouter de mots au titre visible ni utiliser d’annonce vocale forcée.

Version 0.8.3 – 2026-07-23

Mode sombre
• Ajout d’un mode sombre, activable depuis le menu Affichage et enregistré dans les préférences.
• Le thème sombre est appliqué à l’éditeur, aux menus, aux fenêtres secondaires et aux principaux contrôles, avec des couleurs de texte adaptées pour préserver la lisibilité et l’accessibilité.

Langue allemande
• Ajout de l’allemand comme langue complète de l’interface, sélectionnable dans les Options.
• Les actualités et RSS, le correcteur orthographique, le calendrier et toutes les citations, les dons, le guide et le journal des modifications sont entièrement disponibles en allemand.

Portugais brésilien et Google Actualités
• Ajout du portugais brésilien comme langue complète de l’interface, distincte du portugais du Portugal et sélectionnable dans les Options.
• L’interface, le calendrier et toutes les citations, le correcteur orthographique, les dons, le guide et le journal des modifications sont entièrement disponibles en portugais brésilien.
• Google Actualités prend désormais en charge la localisation brésilienne, les catégories du Brésil et des sources RSS brésiliennes par défaut distinctes.
• Lorsque le flux les fournit, les sources Google Actualités liées à un même sujet sont affichées comme éléments enfants accessibles dans l’arborescence.

LibriVox
• La recherche LibriVox a été optimisée afin d’éviter un nombre excessif de requêtes au service et les blocages de l’interface. Les analyses étendues du catalogue ont été supprimées, le nombre de tentatives réduit et des délais d’attente plus courts ont été introduits.

Synthèse vocale
• Les suites de trois points ou plus sont désormais normalisées avant la lecture, afin d’éviter que certaines voix prononcent « point point » ou produisent des segments composés uniquement de ponctuation.

Articles Google Actualités associés
• Pour chaque actualité, des articles associés sont désormais affichés lorsqu’ils sont disponibles, c’est-à-dire d’autres articles traitant de la même information. Pour les lire, il suffit de développer l’article principal lorsque Sonarpad signale que des articles associés sont disponibles. Si vous ne souhaitez pas développer cette section, il suffit d’appuyer sur Entrée sur l’article principal et de lire l’actualité comme d’habitude.
• Les articles associés utilisent désormais le même système lu/non lu que les articles principaux, avec les annonces accessibles, la date et l’heure, l’enregistrement de l’état et sa conservation après l’actualisation des sources ou le redémarrage de Sonarpad.

Annonces dans les parties des livres audio
• Ajout dans les options audio de la liste « Annonce au début de chaque partie ». Pour les livres audio divisés en plusieurs fichiers, chaque partie peut commencer sans annonce, avec le titre du livre, le titre et le numéro de partie, le nom du fichier ou le nom du fichier et le numéro de partie.

Version 0.8.2 – 2026-07-17

Bibliothèques numériques et livres audio
• Ajout de Project Gutenberg, avec recherche par titre ou auteur et sélection de la langue.
• Les livres EPUB de Project Gutenberg sont téléchargés dans Documents\Sonarpad\Documents ; à la fin du téléchargement, Sonarpad demande s’il faut ouvrir immédiatement le livre dans l’éditeur.
• Ajout d’Internet Archive pour rechercher et écouter des collections audio, notamment des émissions de radio anciennes, des discours et de la musique en direct.
• Ajout de LibriVox pour rechercher des livres audio par titre ou auteur et lire directement leurs chapitres avec le même lecteur que celui utilisé pour les podcasts.
• Les trois nouvelles fonctions sont disponibles dans le menu Outils et, lorsque le regroupement des menus est activé, dans la section Lecture.

Transcriptions audio longues
• Correction de la transcription des fichiers audio longs : l’audio est désormais automatiquement découpé en parties de 15 minutes, transcrit une partie à la fois puis réassemblé, ce qui évite les erreurs pouvant survenir avec les enregistrements de longue durée.

YouTube
• Les actions les plus utiles qui n’étaient auparavant accessibles qu’après l’ouverture d’une vidéo YouTube et via le menu Lecture sont désormais également disponibles directement dans le menu contextuel de cette même vidéo, comme « Transcrire l’audio en cours », « Créer une audiodescription avec l’IA » et « Enregistrer le média », pour une utilisation plus simple.
• Ajout de l’option « Copier le lien », également accessible avec Ctrl+C, pour copier dans le presse-papiers l’URL de la vidéo, de la playlist ou de la chaîne YouTube sélectionnée.

Version 0.8.1 – 2026-07-16

Synthèse vocale Google
• Correction du démarrage de Google TTS sur les systèmes Windows où les connexions acceptées par le serveur interne du navigateur héritaient du mode socket non bloquant, provoquant l’erreur 10035 et empêchant les voix téléchargées de parler.
• Sonarpad attend désormais que le moteur WASM de Chrome ou Edge soit entièrement chargé avant l’aperçu de la voix ou la lecture avec F5, évitant l’erreur « Chrome WASM TTS engine was not loaded ».
• Le navigateur caché désactive la traduction des pages et l’accessibilité du moteur de rendu afin d’éviter des annonces comme « Traduire la page » et toute interférence avec les commandes de lecture.
• Le panneau « Voix dans l’éditeur » affiche désormais le bouton « Gérer les voix Google... » lorsque le moteur Google est sélectionné et actualise immédiatement la liste des voix installées à la fermeture du gestionnaire.
• Les avertissements de dépendance affichés lors de la suppression de paquets vocaux Google sont désormais traduits dans toutes les langues de l’interface.

Expérience de mise à jour
• Après une mise à jour automatique, la fenêtre de confirmation avec le journal des modifications s’ouvre après la restauration initiale du focus et reste au premier plan, au lieu de n’apparaître qu’après l’appui sur Tab.

Documents PDF
• Correction des PDF dont le texte intégré contenait des caractères NUL et était tronqué à leur première occurrence lors du chargement dans l’éditeur.
• Lorsque pdf-extract renvoie des caractères NUL intégrés, Sonarpad relance l’extraction avec PDFium ; tout NUL résiduel est supprimé avant l’envoi du texte aux contrôles Windows, afin de conserver le reste du document.

Accessibilité des menus
• Le calcul des mnémoniques à l’exécution a été supprimé : les touches d’accès sont désormais écrites explicitement dans chacune des 15 traductions de l’interface et restent donc identiques à chaque démarrage.
• Toutes les entrées stables des menus principaux et sous-menus ont été vérifiées, notamment Lecture, les polices, Enregistrer l’image et Afficher l’index EPUB ; les mnémoniques manquantes ou dupliquées entre éléments de même niveau ont été corrigées directement dans les traductions.
• Les tests automatiques se contentent désormais de valider les traductions et échouent si une mnémonique manque, est invalide ou est dupliquée ; ils ne modifient plus les libellés à l’exécution.
• Dans les menus exceptionnellement longs où le texte traduit ne fournit pas assez de caractères distincts, une touche d’accès numérique explicite est affichée selon la forme Windows standard « (&1) ».

Version 0.8.0 – 2026-07-15

Dictionnaire en ligne
• Ajout de l’allemand au dictionnaire en ligne Wiktionary.
• Les définitions et synonymes allemands sont désormais reconnus correctement selon la structure propre au Wiktionnaire allemand.

Fiabilité des livres audio SAPI5
• La création de livres audio SAPI5 conserve jusqu’à 12 workers parallèles lorsque la voix sélectionnée produit un résultat fiable.
• Chaque partie est contrôlée selon sa taille, sa durée estimée et une comparaison prudente avec le texte attribué.
• Les parties absentes ou suspectes sont régénérées automatiquement avec une concurrence progressivement réduite : 12, 8, 6, 4, 2 puis 1 worker. Seules les parties problématiques sont répétées.
• La limite fiable est mémorisée séparément pour chaque voix SAPI5, sans ralentir celles qui fonctionnent correctement avec 12 workers.
• Un contrôle final empêche d’accepter silencieusement un MP3 beaucoup plus court que les parties générées.
• Les détails sont enregistrés dans `sapi5_audiobook_diagnostic.log`.
• Chaque unité de synthèse SAPI5 s’exécute désormais dans un processus Sonarpad séparé et invisible. Si une voix tierce plante, seul ce worker se ferme et l’application principale reste ouverte.
• Pendant la même création de livre audio, les parties inachevées sont immédiatement réessayées avec le niveau de concurrence inférieur suivant ; les parties déjà validées sont conservées.
• La récupération au prochain démarrage reste une protection supplémentaire uniquement si l’application principale ou l’ordinateur est interrompu.

Processus de livres audio SAPI4
• Le nombre de processus SAPI4 choisi par l’utilisateur est désormais respecté jusqu’à un maximum technique de 64 ; l’ancienne limite cachée de 16 a été supprimée.
• Le nombre effectif n’est réduit que lorsque le livre audio contient moins d’unités de travail que demandé.
• Si un ou plusieurs processus du pont SAPI4 échouent, les parties terminées sont conservées et seules les unités en échec sont relancées automatiquement avec une concurrence progressivement réduite.
• Sonarpad vérifie maintenant le code de sortie du pont SAPI4 et refuse les parties audio vides ou non valides.

Configuration du proxy
• Ajout d’un champ séparé pour le port du proxy dans les paramètres réseau.
• Le port peut être indiqué indépendamment de l’adresse, il est validé de 1 à 65535 et remplace correctement un port déjà présent dans l’URL.

Recherche de radios par langue et pays
• Les filtres Langue et Pays sont désormais alimentés par toutes les entrées disponibles dans le catalogue Radio Browser et ne sont plus limités à une liste fixe.
• Les noms de langues sont désormais reconnus même lorsque Radio Browser les fournit dans un autre alphabet, sous leur nom natif, sous forme d’abréviation ou comme combinaison de plusieurs langues, puis affichés dans la langue actuelle de l’interface. Les valeurs qui ne correspondent pas à de véritables langues, comme les nombres, genres musicaux, pays ou libellés génériques, sont filtrées.
• Le catalogue est actualisé en arrière-plan, avec une liste de secours utilisable lorsque Radio Browser est inaccessible.
• Les entrées de langue Radio Browser qui deviennent identiques après traduction sont désormais regroupées en un seul élément de la liste, évitant les déplacements silencieux avec les lecteurs d’écran.

Amélioration principale : synchronisation entre la lecture et le curseur
• La synchronisation entre la lecture vocale et le déplacement du curseur a été considérablement améliorée pour tous les moteurs de synthèse vocale pris en charge.
• Lorsque l’option « Déplacer le curseur pendant la lecture » est activée, Sonarpad utilise désormais un système d’avancement commun pour Microsoft Edge Neural, Google TTS, SAPI4, SAPI5 et OneCore.
• Le curseur suit plus précisément le texte réellement prononcé, avec une segmentation plus cohérente des phrases et groupes de mots.
• Les avances, retards, sauts irréguliers et différences entre moteurs vocaux ont été fortement réduits.
• La position correcte est mieux conservée après une pause, une reprise, une recherche dans le document ou un changement de moteur vocal.

Enregistrement de podcast dans des fichiers séparés
• Ajout de l’option « Enregistrer le microphone et l’audio système ou des applications dans des fichiers séparés ».
• Lorsque le microphone et une autre source sont enregistrés ensemble, Sonarpad peut créer un fichier contenant uniquement le microphone et un second contenant l’audio système, une application ou les applications sélectionnées.
• La séparation est disponible en MP3 et en WAV.
• Si l’option est désactivée, Sonarpad continue de créer un seul fichier mixé.
• Les fichiers séparés facilitent le réglage des volumes, la suppression du bruit et le montage ultérieur des podcasts, entretiens et tutoriels.

Enregistrements radio programmés
• Les enregistrements radio peuvent désormais être programmés à l’avance.
• Pour chaque enregistrement, il est possible de choisir la station, le jour, l’heure et les minutes de début ainsi que la durée.
• Une durée personnalisée de 1 à 1 440 minutes est disponible.
• L’enregistrement peut être exécuté une seule fois, chaque jour ou chaque semaine.
• La fenêtre affiche plus clairement les enregistrements actifs et programmés, la date et l’heure prévues, la durée et le temps restant.
• Le Planificateur de tâches Windows peut démarrer automatiquement l’enregistrement même lorsque Sonarpad n’est pas déjà ouvert.

Calendrier
• Ajout d’un calendrier complet et accessible au clavier.
• Il permet de consulter les jours précédents et suivants, de revenir rapidement à aujourd’hui et de connaître les jours fériés et commémorations.
• Ajout du saint du jour et de la citation du jour, qui peuvent être lus, prononcés ou copiés.
• Les rappels peuvent être créés, modifiés, supprimés, reportés et marqués comme terminés.
• Les alertes peuvent être affichées à l’heure exacte ou à l’avance et utiliser la planification Windows même lorsque Sonarpad est fermé.

Météo
• Ajout d’une section consacrée aux prévisions météorologiques.
• Il est possible de rechercher une ville et de retrouver rapidement les lieux récemment consultés.
• Les conditions actuelles, la température, les valeurs minimale et maximale, l’humidité, la probabilité de précipitations et les prévisions des jours suivants sont disponibles.
• Les températures peuvent être affichées en Celsius, Fahrenheit ou automatiquement.

Films au cinéma
• Ajout d’une section pour les films actuellement en salles et les prochaines sorties.
• Recherche par titre, résumé, date de sortie et lecture de la bande-annonce sont disponibles.

Synthèse vocale Google
• Intégration de Google TTS pour la lecture des documents et la création de livres audio.
• Ajout d’un gestionnaire permettant d’afficher les voix, de les filtrer par langue, de les télécharger et de supprimer celles qui ne sont plus nécessaires.
• La vitesse, le volume et la hauteur peuvent être réglés.
• La hauteur des voix Google Natural est appliquée directement par le moteur pour un résultat plus naturel et plus stable.
• La réactivité et la fiabilité de Google TTS ont été améliorées, avec des délais adaptés à la vitesse choisie.
• Les attentes inutiles ont été réduites et la gestion des erreurs et interruptions a été améliorée.

Table des matières EPUB
• Sonarpad reconnaît désormais la table des matières intégrée aux livres EPUB.
• Sa présence est annoncée et elle peut être ouverte depuis le menu Affichage.
• Les chapitres et sous-chapitres sont présentés hiérarchiquement.
• Appuyer sur Entrée permet d’atteindre immédiatement l’emplacement sélectionné.

Actualités et sources RSS
• La section Actualités a été enrichie de nouveaux outils de recherche et d’organisation.
• Ajout du choix de la langue des actualités.
• Il est possible d’effectuer une recherche dans les sources RSS et de consulter les actualités de sa ville.
• Les sources communautaires peuvent être parcourues, ajoutées à sa collection et proposées à la communauté Sonarpad.

Enregistrement de podcast
• Il est possible d’enregistrer uniquement le microphone, tout l’audio système, une application, plusieurs applications sélectionnées, ou le microphone et les applications ensemble.
• Le périphérique du microphone et la source audio peuvent être choisis, les volumes réglés séparément et les niveaux surveillés en temps réel.
• Ajout de la pause et reprise, des formats MP3 ou WAV, du choix du débit MP3 et du dossier de destination.
• L’ordinateur peut rester actif pendant l’enregistrement.

Radio
• La section Radio a été profondément réorganisée.
• Les stations peuvent être recherchées par nom ou texte libre, langue, pays, ville, genre musical ou catégorie.
• La gestion des favoris a été améliorée et tous les filtres peuvent être réinitialisés rapidement.
• Les stations peuvent être proposées à la communauté Sonarpad.
• Ajout de l’enregistrement en direct, du mode « Enregistrer et lire », de la liste des enregistrements ainsi que de leur gestion et suppression.
• Les enregistrements radio sont conservés dans leur propre dossier au sein du répertoire général des enregistrements.

Lecture multimédia
• La stabilité du lecteur multimédia a été considérablement améliorée.
• Correction d’un problème pouvant bloquer mpv et amélioration de la communication avec le lecteur.
• Amélioration de l’ouverture des différents types de fichiers multimédias.
• Sonarpad mémorise désormais le volume utilisé pendant la lecture.
• Amélioration de la gestion des flux et des enregistrements.
• Correction de l’ouverture depuis Windows par double-clic ou « Ouvrir avec ».

Documents PDF
• Ajout de la reconnaissance des champs de formulaire dans les PDF.
• Sonarpad peut repérer les champs à remplir, les présenter sous une forme textuelle accessible, permettre leur modification et enregistrer les données dans le PDF.
• Correction du calcul de la position du curseur pendant la lecture, notamment avec les caractères multioctets et les structures complexes.

Accessibilité et clavier
• Amélioration des commandes d’édition standard dans l’ensemble du programme.
• Copier, couper, coller, tout sélectionner, annuler et rétablir sont correctement envoyés au champ ayant le focus, y compris dans les fenêtres secondaires et les boîtes de dialogue.
• Correction d’un problème de mise à jour des afficheurs Braille.
• Amélioration de la gestion du focus et correction du choix de langue dans Wikipédia.
• Ajout du regroupement par catégorie des fonctions du menu Outils.
• Ajout d’actions configurables pour ouvrir rapidement Calendrier, Météo et Films au cinéma.

Livres audio
• Amélioration de la création des livres audio lorsque des boîtes de dialogue ou fenêtres modales sont ouvertes.
• La progression est plus robuste et ignore les anciennes mises à jour audio.
• Google TTS peut également être utilisé pour créer des livres audio avec réglage de la vitesse, du volume et de la hauteur.

Intelligence artificielle
• Mise à jour du modèle Gemini par défaut vers `gemini-3.5-flash`.

Corrections générales
• Correction de plusieurs blocages pendant la lecture avec mpv.
• Correction de l’ouverture de certains fichiers audio et vidéo.
• Amélioration des commandes envoyées au lecteur.
• Correction du rétablissement du curseur pendant la lecture vocale.
• Amélioration de la stabilité de la création des livres audio.
• Amélioration générale de la gestion des médias, RSS, radio et EPUB.

Version 0.7.1 – 2026-05-13

Nouveautés et améliorations
• Création du site officiel sonarpad.com, un nouveau point de référence pour suivre les dernières nouveautés, télécharger la dernière version du programme, lire les commentaires des visiteurs et, à l’avenir, écouter également tous les podcasts de Sonarpad. L’entrée « Visiter sonarpad.com » a également été ajoutée au menu Aide, afin d’ouvrir rapidement le site officiel.
• Correction du problème qui provoquait une erreur avec les fichiers contenant des accents ou des caractères spéciaux lors du lancement de la transcription vocale.
• Désormais, dans le menu Affichage, les options comme Retour automatique à la ligne et Afficher la vidéo pendant la lecture afficheront toujours leur état correct, activé ou désactivé.
• Amélioration de la recherche YouTube, avec la possibilité de revenir à la page ou à l’écran précédent avec Échap.
• Ajout d’une vérification préliminaire pour contrôler si une vidéo peut être lue. La lecture a également été améliorée : Sonarpad peut désormais lire les vidéos ou playlists marquées comme mix, qui ne pouvaient pas être lues auparavant.
• Amélioration de la gestion des signets automatiques. Auparavant, si l’option Signets automatiques était activée puis désactivée, ces signets restaient ; désormais le programme les ignore correctement jusqu’à ce que l’option soit réactivée. De plus, lorsque la fin d’un fichier multimédia est atteinte, le signet est supprimé automatiquement.
• Amélioration de la gestion des balises lorsque les dialogues sont activés. Sonarpad gère désormais correctement les deux fonctions, permettant d’insérer des balises même lorsque l’option dialogues est active.
• Amélioration des paramètres vocaux, avec une séparation claire de chaque moteur afin de rendre les réglages plus précis. Les profils vocaux conservent correctement les paramètres de chaque moteur : Edge, Sapi5 et Sapi4.
• Ajout d’une balise pour insérer des pauses, directement depuis les options ou depuis le panneau des voix en appuyant sur Tab depuis l’éditeur. Les choix disponibles sont : 250 ms, 500 ms, 1 seconde, 2 secondes ou durée personnalisée.
• Correction du comportement lors de la lecture d’une vidéo YouTube et du lancement de la transcription. Désormais, en revenant avec Alt+Tab, le focus sera correctement placé sur le bouton Annuler de la transcription en cours.
• Les transcriptions sont désormais enregistrées automatiquement à la fin du processus.
• Amélioration de l’importation depuis Wikipédia. Il est possible de choisir de lire seulement une section puis, depuis l’article, de revenir à la recherche avec Échap, ou bien d’importer l’article complet. Il est également possible de choisir la langue de Wikipédia à consulter.
• Ajout d’une section de radios du monde entier, où il sera possible de rechercher une radio par pays, langue et genre. Il sera également possible d’ajouter des radios locales à la base de données de Sonarpad, afin que d’autres utilisateurs puissent les écouter. Il est aussi possible d’ajouter une radio aux favoris.
• Ajout d’une section d’itinéraires pour calculer des parcours en choisissant le mode : à pied, à vélo, en voiture ou en fauteuil roulant. Il est possible de choisir l’itinéraire le plus court ou le plus rapide et d’afficher les communes traversées. Une fois l’itinéraire importé, il sera aussi possible d’enregistrer la carte visuelle depuis le menu Fichier, Enregistrer l’image.
• Ajout de l’option Imprimer dans le menu Fichier. Sonarpad imprimera les fichiers TXT avec son propre système et utilisera le programme associé pour les autres fichiers, comme DOCX, PDF et formats similaires, afin de préserver autant que possible la mise en page originale.
• Intégration dans Sonarpad d’un service de traduction pour chaque document, accessible depuis le menu contextuel de l’éditeur. L’utilisateur pourra utiliser gratuitement DeepL et Google Translate sans saisir de clé API ; en saisissant une clé API Gemini, il pourra traduire avec Gemini.
• Dans le menu de traduction, l’utilisateur pourra choisir la langue de destination. Le menu se réorganise automatiquement : si un utilisateur choisit d’abord l’anglais, puis le français et enfin l’italien, ces trois options apparaîtront en haut du menu des langues.
• Si l’utilisateur saisit sa clé API Gemini, il pourra également accéder à la fonction Résumer le texte, toujours disponible dans le menu contextuel, pour résumer n’importe quel article.
• Ajout dans le menu Lecture, visible pendant la lecture d’un fichier multimédia, d’un menu permettant de diviser le média en cours. Il fonctionne avec MP3, MP4 et d’autres formats, en divisant par nombre de parties ou selon la durée de chaque partie.

Version 0.7.0 – 2026-04-25

Nouveautés
• Ajout de la prise en charge du lecteur mpv pour la lecture en streaming. Les vidéos provenant de YouTube et des sites compatibles sont désormais lues immédiatement ; si l'utilisateur choisit de les conserver, elles sont téléchargées comme auparavant. Lors de la transcription d'un contenu en streaming, celui-ci est d'abord téléchargé puis transcrit. Le lecteur mpv est également utilisé pour ouvrir des vidéos locales et gérer les sous-titres, garantissant une meilleure compatibilité avec de nombreux formats auparavant mal pris en charge.
• Amélioration de l’enregistrement de podcasts pour l’audio du système : il est désormais possible de choisir entre l’enregistrement de tout l’audio du système, d’une seule application ou de plusieurs applications en même temps. Cette option est intégrée à l’enregistrement normal, il reste donc possible d’activer ou de désactiver le microphone séparément.
• Ajout de la langue hindi. Interface traduite et ajout des flux RSS, du journal des modifications et du guide Sonarpad.
• Ajout d'une option dans l'onglet Éditeur pour déplacer toujours le curseur au début de la ligne avec les flèches haut et bas.
• Ajout d'une option dans le menu "Convertir audio" pour convertir l'audio en M4B.

Corrections
• Dans les commentaires YouTube ouverts depuis « Lire l'audio en streaming... », Sonarpad charge maintenant au départ seulement les 50 premiers commentaires principaux, en incluant toujours toutes les réponses à ces commentaires, et ajoute à la fin une entrée permettant de charger tous les commentaires à la demande.
• Les signets sont désormais affichés et gérés selon leur position, aussi bien dans les documents texte que dans les fichiers multimédias, au lieu de suivre l'ordre de création. Si un signet existe déjà à la même position, il n'est plus ajouté une seconde fois.
• Ajout d'une option dans le menu Signets qui, lorsqu'elle est activée, permet une gestion automatique des signets. Lorsqu'un fichier local ou en streaming est lu puis fermé, Sonarpad crée automatiquement un signet en fonction de la position atteinte et, à la réouverture du fichier, reprend à partir de ce point. Il en va de même pour les fichiers texte : si un texte est ouvert et que le curseur est déplacé, Sonarpad mémorisera cette position à la fermeture ; si la lecture est lancée, la dernière phrase lue sera enregistrée et la lecture reprendra exactement à cet endroit.
• Ajout d'une entrée dans le menu Affichage pour afficher le rendu vidéo des fichiers locaux ou en streaming. Le contenu vidéo est affiché dans une fenêtre agrandie, où toutes les commandes sont masquées, sauf si l'on appuie sur la touche Alt ou si l'on déplace la souris vers la partie supérieure de la fenêtre. Ainsi, les utilisateurs malvoyants devraient bénéficier d'un contenu plus grand et plus facile à utiliser.

Version 0.6.9 – 2026-04-08

Corrections
• L'expérience de Rechercher dans les fichiers a été améliorée : lors de l'ouverture de Parcourir le dossier, le focus va directement sur la liste des dossiers ; l'ouverture d'un résultat avec Entrée ne bloque plus les commandes clavier ; avec Échap, on revient au résultat précédemment sélectionné ; et en revenant avec Alt+Tab, le focus revient soit au champ de recherche, soit à la liste des résultats si elle était ouverte.
• F5 lançait toujours la lecture depuis le début. Cela a été corrigé et la lecture démarre désormais à la position actuelle du curseur, tout en conservant `Shift+F5` et `Ctrl+F5` pour aller à la phrase précédente ou suivante.
• Après avoir utilisé Aller à la ligne, appuyer sur Esc pouvait faire perdre le focus à Sonarpad. Le focus revient maintenant correctement dans l’éditeur.
• L’option `Retour à la ligne automatique` s’applique désormais immédiatement aussi aux documents déjà ouverts, sans devoir rouvrir le fichier.

Version 0.6.8 – 2026-04-07

Nouveautés
• Ajout d’un nouvel élément dans le menu Lecture permettant de transcrire n’importe quel fichier audio ou vidéo avec Whisper. Une nouvelle section « IA et transcription » est désormais disponible dans Options, où vous pouvez choisir le modèle, activer la prise en charge optionnelle de CUDA pour les cartes graphiques NVIDIA, conserver la langue d’origine et activer ou désactiver les horodatages.
• Ajout dans le menu Lecture de la nouvelle action `Transcrire le dossier actuel`, qui transcrit tous les fichiers audio pris en charge du dossier du média ouvert et les regroupe dans un document unique, avec une fenêtre de progression dédiée, l’indication du fichier en cours et la possibilité d’annuler. Elle peut aussi être lancée avec `Alt+Shift+C`.
• Ajout de la dictée vocale hors ligne, avec le même fonctionnement que la transcription audio. Par défaut, appuyez sur `Ctrl+Shift+Espace` pour démarrer la dictée puis appuyez à nouveau sur le même raccourci pour l’arrêter ; ce raccourci peut être personnalisé dans les Options. À partir de la deuxième activation, la dictée est plus rapide car le moteur reste prêt en mémoire ; sur les PC disposant de moins de 4 Go de RAM, ce préchargement et cette réutilisation sont désactivés automatiquement.
• Ajout dans les Options de l’éditeur d’un nouveau réglage, désactivé par défaut, qui permet à `Esc` de fermer la fenêtre de l’éditeur.
• La recherche de podcasts utilise désormais `iTunes + Spreaker` par défaut, avec filtrage des doublons lorsque le même podcast est présent sur les deux plateformes.
• Amélioration de la recherche et de l’exploration des podcasts Apple : la recherche de podcasts, la navigation par catégorie et les top podcasts par catégorie utilisent désormais le pays sélectionné pour le répertoire de podcasts. Dans Options > RSS / Podcast, vous pouvez laisser `Automatique` pour utiliser le pays du système ou choisir manuellement un autre pays.
• La limite de résultats pour les catégories de podcasts Apple a été augmentée. À la première ouverture, Sonarpad charge toujours les 50 premiers résultats comme avant ; si vous choisissez `Charger plus de résultats`, Sonarpad charge jusqu'à 200 résultats au total (limite imposée par Apple) et permet de naviguer dans les pages suivantes tout en gardant une expérience fluide.
• Sonarpad est désormais disponible aussi sur Mac, même avec un ensemble de fonctions partiel. Lien du projet : https://github.com/Ambro86/Sonarpad-Mac

Améliorations
• Plus de 50 pays sélectionnables ont été ajoutés pour le répertoire de podcasts, afin de pouvoir choisir parmi un éventail bien plus large de catalogues nationaux.
• « Lire l'audio en streaming... » permet désormais aussi d'effectuer une recherche YouTube à partir de n'importe quel texte, ou de coller le lien d'une chaîne ou d'une playlist YouTube pour afficher ses résultats.
• L’affichage des résultats dans « Lire l'audio en streaming... » a été amélioré : les entrées YouTube incluent maintenant le titre, la durée, la chaîne et le nombre de vues dans un format plus clair.
• « Lire l'audio en streaming... » prend désormais aussi en charge les commentaires YouTube : ils peuvent être ouverts depuis le menu contextuel, les réponses peuvent être lues et les fils de commentaires peuvent être développés avec la Flèche droite.
• Ajout des favoris YouTube pour les chaînes et les playlists dans « Lire l'audio en streaming... » : ils peuvent être ajoutés depuis les résultats via le menu contextuel, ouverts directement depuis la liste Favoris accessible avec Tab juste après le champ URL/requête YouTube, puis supprimés depuis cette même liste avec le menu contextuel. Dans les résultats de recherche YouTube, le menu contextuel n’est disponible que pour les chaînes et les playlists.
• « Lire l'audio en streaming... » peut désormais demander des identifiants lorsqu’un site exige une connexion. L’utilisateur peut les saisir, les enregistrer pour ce site et gérer ensuite les identifiants enregistrés dans Options > Audio.
• Amélioration du focus pendant « Lire l'audio en streaming... », afin que la fenêtre de progression reste plus stable pendant le téléchargement et la conversion.
• Ajout de deux nouvelles actions de lecture dans le menu Voix : `Phrase précédente` et `Phrase suivante`, avec des raccourcis configurables pour se déplacer pendant la lecture du texte.
• Le raccourci par défaut de `Exécuter le fichier avec l’interpréteur` est maintenant `Ctrl+Shift+F5`, afin que `Shift+F5` puisse être utilisé par défaut pour `Phrase précédente`.
• Ajout de la gestion des profils vocaux dans Options > Voix : il est possible d'ajouter, de renommer et de supprimer des profils.
• Extension dans Options > Audio des choix pour l’intervalle de retour arrière pendant la lecture, avec de nouvelles valeurs allant de 1 seconde à 2 heures.
• Ajout de la traduction russe grâce à Dmitriy.
• Ajout dans Options > Audio d’un nouveau choix pour le format du nom des parties du livre audio : `Titre + numéro`, `Numéro uniquement` ou `Numéro + titre`.
• Ajout dans le menu contextuel des articles RSS de l'action pour ajouter un article aux favoris.
• La source RSS "Favoris" peut être supprimée et est recréée automatiquement lors du prochain ajout d'un article aux favoris.
• Ajout de raccourcis clavier RSS pour déplacer les sources vers le haut/le bas : `Ctrl+Shift+Flèche haut` et `Ctrl+Shift+Flèche bas`.
• Amélioration de la fenêtre RSS avec un aperçu d'article intégré, afin de consulter directement le texte dans la fenêtre et d'y accéder rapidement avec Tab avant d'ouvrir l'article complet dans l'éditeur.
• Ajout dans RSS d’une entrée explicite « Charger plus d’actualités » à la fin des sources lorsque d’autres éléments sont disponibles ; en appuyant sur Entrée, le bloc suivant est chargé et le focus se déplace vers le premier nouvel article.
• Dans le dictionnaire vocal, lors de l’ajout ou de la modification d’un remplacement, une case « Respecter la casse » permet désormais de choisir si chaque substitution doit respecter ou ignorer les majuscules/minuscules.
Correctifs
• « Lire l'audio en streaming... » respecte désormais la limite de cache des podcasts déjà définie dans les Options, et cette même limite s'applique aussi à la lecture des audiodescriptions.
• Correction de l’importation depuis Wikipédia, qui sur certaines pages n’importait pas correctement les citations présentes dans le texte.
• Amélioration de l’analyseur de pages web : sur certaines pages WordPress, les éléments de liste et certains titres de section n’étaient pas inclus.
• Désormais, lorsque l’on utilise « Aller à la ligne », le champ est prérempli avec la ligne actuelle.
• Correction de l’export OPML des podcasts et des flux RSS : les fichiers générés sont désormais acceptés par iTunes.
• Correction de la transcription des fichiers multimédias : désormais, lorsque l’on ferme avec Alt+F4 le document généré, Sonarpad demande s’il faut l’enregistrer et propose le bon nom en se basant sur le nom du fichier transcrit au lieu de la première ligne du texte.
• Ajout de messages de confirmation localisés pour l’importation et l’exportation OPML corrects des flux RSS et des podcasts.
• Correction d’un problème où, dans « Lire l'audio en streaming... », après avoir saisi une recherche textuelle et sélectionné une chaîne YouTube dans les résultats, le programme pouvait sembler bloqué au lieu d’ouvrir les vidéos de la chaîne.
• Correction d’un bug où la liste des fichiers ouverts s’affichait dans le menu Aide au lieu du menu Fenêtre.
• Correction d’un cas limite de streaming où la lecture pouvait démarrer mais la fenêtre « Téléchargement du flux » restait ouverte lorsque le fichier téléchargé correspondait déjà au format cible.
• Correction du comportement de conversion en streaming MP3 : lorsque le flux est déjà en MP3 et que l’utilisateur choisit un bitrate MP3 explicite (par exemple 128 kbps), Sonarpad réencode désormais au bitrate sélectionné au lieu d’ignorer la conversion.
• Correction du raccourci `Alt+Shift+L` : il ouvre désormais correctement la liste des chapitres pendant la lecture.
• Correction du raccourci `Alt+Shift+T` : il lance désormais correctement « Transcrire l’audio en cours » au lieu d’ouvrir le menu Outils.
• Si un audio est déjà en cours de lecture, Sonarpad le met désormais automatiquement en pause avant de démarrer la transcription.
• Correction d’un problème où l’import d’un article depuis Wikipédia pouvait réussir sans afficher le texte de l’article à l’écran.
• Ajout de la prise en charge des chapitres de podcast intégrés dans les fichiers multimédias locaux (par ex. métadonnées de chapitres MP3) : lorsque le flux/URL ne fournit pas de chapitres, Sonarpad les charge désormais depuis le fichier téléchargé en arrière-plan, ce qui permet de démarrer la lecture immédiatement puis d’appliquer les chapitres dès qu’ils sont disponibles.
• Correction du chargement des chapitres pour les épisodes de podcast téléchargés et ouverts comme de simples fichiers multimédias locaux : les chapitres intégrés sont désormais disponibles aussi dans ce cas, et pas seulement quand la lecture démarre depuis la fenêtre Podcasts.
• Correction de la finalisation des livres audio MP3 avec SAPI4 et SAPI5 : le fichier final est désormais finalisé correctement, ce qui évite les fichiers incomplets ou fragiles après de longues exportations.
• Ajout d’une barre de progression explicite pour la phase de finalisation dans tous les modes de création de livres audio : après la phase de création, Sonarpad annonce et affiche la finalisation avec une progression visible.
• Correction d’un bug des voix de dialogue : les paramètres vitesse/tonalité/volume de la première et de la seconde voix de dialogue sont désormais correctement appliqués pendant la synthèse.
• Amélioration de la détection d’encodage pour les fichiers japonais `.txt` : ajout d’un fallback Shift_JIS/CP932 sûr en cas de mojibake, tout en préservant le comportement existant pour UTF/diacritiques/chinois.
• Refactorisation interne de sûreté : conversion vers des implémentations safe lorsque possible et réduction drastique des lignes de code unsafe.

Version 0.6.7 – 2026-03-02
Améliorations
• Ora il programma riesce a gestire Sostituisci tutto in modo massivo su file grandi con un gran numero di sostituzioni.
• Mise à jour de la traduction polonaise grâce à DJ Graco.
• Ajout de la traduction lituanienne.
• Ajout de la traduction chinoise.
• À partir de maintenant, des versions bêta fréquentes seront publiées dans la section Releases du projet, afin que les utilisateurs puissent tester les nouvelles modifications avant la prochaine version stable.
• Ajout du raccourci `Ctrl+.` pour insérer le caractère de points de suspension (…).
• Amélioration de la prise en charge des chapitres de podcast : la navigation entre chapitres est désormais plus fiable, y compris pour les épisodes directs/streaming où les chapitres ne sont pas intégrés au fichier MP3, grâce à un fallback sur les métadonnées de chapitres du flux/URL lorsqu’elles sont disponibles. Ajout des raccourcis `Ctrl+Alt+PageUp` (chapitre précédent) et `Ctrl+Alt+PageDown` (chapitre suivant).
• Réorganisation des dossiers de sortie dans `Documents\\Sonarpad` : les fichiers sont désormais enregistrés dans des sous-dossiers dédiés `audiobooks`, `documents`, `recordings` et `media`, avec migration automatique depuis les anciens chemins.
• Amélioration de la prise en charge des fichiers texte très volumineux (jusqu’à 60 Mo) : ouverture et navigation ligne par ligne plus fluides, en particulier avec les lecteurs d’écran.
• Guides mis à jour pour toutes les langues et ressources de localisation actualisées dans toute l’application, y compris les textes de dons et les traductions de l’installateur NSIS (nouvelles chaînes en chinois simplifié et lituanien, ainsi que finalisation de la traduction ukrainienne du setup).
• Ajout de la prise en charge globale du proxy réseau (HTTP/HTTPS et SOCKS5/SOCKS5H) pour les fonctions en ligne, avec validation à l'enregistrement des options : les proxys invalides sont signalés puis supprimés automatiquement.
• Ajout d'une nouvelle fonction dans Outils : « Lire l'audio en streaming... », permettant de coller une URL (YouTube ou lien média direct), de choisir le format de sortie et le profil qualité/débit binaire (y compris qualité/débit d'origine pour MP3 et MP4), puis de lancer la lecture dans le lecteur audio de Sonarpad.
• Ajout de la prise en charge de la touche multimédia système Lecture/Pause (casque/clavier) : elle contrôle désormais à la fois la lecture multimédia et la pause/reprise de la lecture de texte (priorité au lecteur multimédia lorsque les deux sont actifs).
• Ajout d'une nouvelle entrée dans Fichier > Fichiers récents : « Vider les fichiers récents » pour effacer rapidement la liste des documents récents.
• Extension des options de débit binaire dans Convertir l’audio et dans les paramètres d’enregistrement de podcast : ajout de valeurs plus basses (64/96 kbps) et extension du MP3 jusqu’à 320 kbps, avec validation et gestion de l’encodeur harmonisées.
• Extension des options de découpage de livre audio par durée jusqu’à 60 minutes.
• Amélioration du découpage en parties : il est désormais possible de saisir manuellement le nombre de parties, avec validation de 1 à 100.
• Ajout du nouveau mode Affichage > Lecture seule pour éviter les modifications accidentelles tout en conservant une lecture et une navigation complètes des documents.
• Ajout d’une barre de progression accessible pendant les mises à jour du programme, afin que les lecteurs d’écran puissent suivre en temps réel l’avancement du téléchargement.
• Ajout d’une nouvelle barre d’état discrète dans la fenêtre principale avec le nombre de caractères, de mots et la position ligne/colonne (par exemple : "Caractères (avec espaces) : 11. | Mots : 2. | Ln 1, Col 12"), sans perturber le focus NVDA.
• Ajout d’une nouvelle option dans le menu Affichage pour le retour à la ligne, afin d’activer ou désactiver rapidement l’habillage sans ouvrir les Options.
• Ajout, dans Édition > Texte, de nouvelles actions pour augmenter/réduire le retrait, avec les raccourcis Ctrl+Shift+. (indenter) et Ctrl+Shift+, (désindenter), car lorsque « Afficher les voix dans l’éditeur » est activé, la touche Tab est réservée à la navigation du panneau des voix.
• Ajout de l'affichage localisé de la date et de l'heure dans les articles RSS et les épisodes de podcast, avec un format adapté à la langue de l'interface.
• Ajout d'une nouvelle action dans le menu contextuel RSS pour partager l'article sélectionné par e-mail.
• Ajout d'options granulaires de confirmation de suppression dans Options > RSS et podcast : pour RSS (flux/article/les deux/aucun) et pour Podcasts (podcast/épisode/les deux/aucun).
• Ajout d'une copie rapide RSS configurable avec Ctrl+C (Options > RSS et podcast) : copier le titre, l'URL, le contenu de l'article ou l'ensemble.
• Unification du flux RSS : « Ajouter une source » accepte désormais à la fois les URL de flux et les mots-clés (avec génération automatique d'un flux Google News), sans recherche séparée.
• Un appui sur Ctrl+A annonce désormais la fin de l'action pour un retour plus clair avec les lecteurs d'écran.
• Ajout de Shift+F3 pour "Rechercher le précédent" dans le menu Édition, en complément de F3 "Rechercher le suivant".
• Amélioration du message de remplacement avec une gestion correcte du singulier/pluriel (par ex. « 1 remplacement effectué » vs « 2 remplacements effectués »).
• Ajout dans la fenêtre du dictionnaire d'une sélection de langue de recherche, avec Auto (langue de l'interface) par défaut et possibilité de choix manuel.
• Ajout d'un nouvel onglet Raccourcis dans les Options pour personnaliser les combinaisons de touches, avec détection des conflits et avertissement lorsqu'un raccourci est déjà attribué à une autre action.
• Ajout d’un support initial des paramètres en ligne de commande : `-h`/`--help` affichent l’aide rapide et `--version` affiche la version du programme.
• Réglage manuel de la vitesse et de la tonalité rendu plus clair : les champs manuels utilisent désormais une échelle centrée sur 100, où 100 correspond à la valeur normale.
• Amélioration de la sélection des voix Microsoft dans Options > Voix et dans le panneau des voix de l’éditeur : ajout d’une liste de langue localisée pour filtrer les voix par langue, tout en conservant le mode « voix multilingues uniquement » comme une liste unique non séparée par langue (liste de langue masquée lorsqu’il est activé).
• Ajout de la configuration de la voix pour les dialogues dans Options > Voix avec navigation complète au clavier (Tab), en réutilisant le même modèle de voix que l’interface principale (moteur, filtre de langue Edge, voix et vitesse/tonalité/volume avec libellés) ; ajout également d’une deuxième voix de dialogue optionnelle avec les mêmes contrôles (moteur, filtre de langue Edge, voix, vitesse/tonalité/volume) pour alterner les dialogues ; les règles de dialogue sont enregistrées dans la configuration `.ini`, sans modifier le texte du document.
• Amélioration de l'étiquette Annuler : l'entrée Édition > Annuler affiche désormais l'action qui sera annulée (par exemple édition de texte, commenter/décommenter des lignes ou insertion de balise de voix), tout en restant indisponible lorsqu'il n'y a rien à annuler.
Améliorations
• Ajout de la gestion des profils vocaux dans Options > Voix : il est possible d'ajouter, de renommer et de supprimer des profils.
• Ajout dans le menu contextuel des articles RSS de l'action pour ajouter un article aux favoris.
• La source RSS "Favoris" peut être supprimée et est recréée automatiquement lors du prochain ajout d'un article aux favoris.
• Ajout de raccourcis clavier RSS pour déplacer les sources vers le haut/le bas : `Ctrl+Shift+Flèche haut` et `Ctrl+Shift+Flèche bas`.
Corrections de bugs
• Correction de l'ouverture des fichiers RTF : les fichiers `.rtf` sont désormais extraits et affichés en texte lisible, au lieu du balisage RTF brut (ex. `{\\rtf1...}`).
• Correction de l'ouverture des fichiers texte chinois encodés en GB18030/GBK : Sonarpad les détecte et les décode correctement, évitant le texte illisible (mojibake).
• Amélioration de la création des livres audio M4B avec métadonnées de chapitres et marqueurs de chapitre ; correction du problème « chipmunk » (voix trop aiguë/rapide) dans les fichiers M4B générés.
• Correction de l’interface bitrate dans la fenêtre d’enregistrement de livre audio : suppression des libellés italiens codés en dur et ajout de 64 kbps dans les débits sélectionnables.
• Correction de « Enregistrer tout » (Ctrl+Shift+S) : tous les documents ouverts modifiés sont désormais détectés de façon fiable (y compris les onglets nouveaux/non enregistrés) et l’enregistrement global fonctionne correctement pour chacun, avec « Enregistrer sous » lorsque nécessaire.
• Correction de l'ordre des articles RSS Google News : lorsque la date est disponible, les articles sont désormais affichés du plus récent au plus ancien.
• Correction de l'association des étiquettes NVDA dans la fenêtre du dictionnaire : le champ de recherche et la liste de langue annoncent désormais la bonne étiquette.
• Correction de la navigation clavier dans la fenêtre Propriétés RSS/Podcast : Tab/Maj+Tab atteignent désormais le bouton OK, Entrée active OK, Échap ferme la fenêtre en toute sécurité et le focus revient correctement à la liste RSS/Podcast.
• Correction de l'historique d'annulation RSS/Podcast : Ctrl+Z prend désormais en charge une annulation multi-niveaux pour les suppressions (articles/épisodes et sources), et plus seulement la dernière action.
• Amélioration des annonces de suppression RSS/Podcast avec des messages explicites (flux RSS supprimé, article RSS supprimé, épisode de podcast supprimé).
• Amélioration du focus après suppression/annulation dans RSS/Podcast : en RSS, le premier flux est sélectionné de manière fiable si nécessaire, et les répétitions d'annonces du lecteur d'écran ont été réduites pendant la resélection différée.

Version 0.6.6 – 2026-02-13
Améliorations
• Ajout de « Formatage automatique pour TTS » dans le menu Édition pour préparer rapidement le texte à la lecture vocale (suppression markdown/guillemets et recomposition des lignes coupées).
• Amélioration de l’insertion des balises de voix : lorsqu’un texte est sélectionné, les balises sont désormais appliquées correctement aussi bien sur une seule ligne que sur une sélection multiligne.
• Ajout d’une option dans les paramètres Audio pour choisir le dossier par défaut d’enregistrement des livres audio (par défaut : Documents\\Sonarpad Audiobooks).
• Dans la fenêtre d’enregistrement du livre audio, lorsque le découpage est actif, ajout d’une nouvelle option (activée par défaut) pour créer un sous-dossier dédié aux parties générées.
• L’export des livres audio enregistre désormais les MP3 en stéréo avec un bitrate choisi par l’utilisateur pour les voix Edge, SAPI5 et SAPI4.
• Ajout de la prise en charge des voix SAPI5 32 bits via bridge, afin d’utiliser aussi les voix disponibles uniquement dans les moteurs 32 bits.
• Réorganisation des fonctions vocales dans un menu dédié « Voix et audio » et ajout/clarification de l’option « Convertir l’audio », utile pour convertir tout média pris en charge en MP3, AAC, OGG, Opus, FLAC, WAV et AIFF.
• Ajout de la suppression d’articles RSS individuels et d’épisodes de podcast individuels (touche Suppr + menu contextuel avec confirmation), sans supprimer toute la source RSS/podcast, avec annulation de la dernière suppression (article/épisode individuel ou source RSS/podcast complète).
• Ajout de l'export des flux RSS en OPML dans la fenêtre RSS, afin de sauvegarder et réimporter facilement les sources actuelles.
• Ajout de la fonction « Rechercher un flux RSS par mot-clé » dans la fenêtre RSS : en saisissant un mot-clé, l'URL RSS Google News est générée automatiquement et la fenêtre d'ajout de source s'ouvre préremplie, afin de créer un flux thématique en une seule étape.
• Ajout de la traduction serbe grâce à Mila Kuran.
• Ajout de la traduction ukrainienne grâce à Ivan Shtefuriak.
• Ajout de l'ouverture multiple de fichiers média : en ouvrant plusieurs fichiers à la fois, une file de lecture est créée au lieu de remplacer le fichier en cours.
• Ajout de raccourcis de déplacement variable pendant la lecture : avec une base de 1 minute, Gauche/Droite avance-recule de 60 s, Maj+Gauche/Droite de 20 s et Ctrl+Gauche/Droite de 3 minutes.
• Ajout des raccourcis piste précédente/suivante dans le lecteur : Ctrl+PageUp et Ctrl+PageDown.
• Ajout de l'option « Réinitialiser le volume » et regroupement des actions de réinitialisation dans un sous-menu dédié « Réinitialiser » dans Lecture, avec « Réinitialiser la vitesse » et « Réinitialiser la tonalité ».
• Amélioration de l'installateur : setup.exe permet désormais de choisir entre associer tous les types de fichiers pris en charge ou sélectionner manuellement les extensions ; le MSI propose aussi une sélection extension par extension dans l'arborescence des fonctionnalités (valeur par défaut inchangée : tout activé).
• Ajout du nouveau menu « Fenêtre » avec l'option « Documents ouverts... » pour basculer rapidement vers n'importe quel fichier actuellement ouvert.
• Mise à jour de l'option Affichage > Police : le sélecteur complet a été remplacé par un sous-menu rapide de polices courantes (Arial, Calibri, Consolas, Segoe UI, Tahoma, Verdana, Times New Roman, Georgia), tout en conservant la taille de texte actuelle.
• Amélioration de la lecture RSS/podcasts avec deux annonces distinctes : les nœuds de source annoncent « nouveaux éléments » lorsqu’un flux/podcast a des nouveautés, tandis que les articles RSS et épisodes de podcast individuels annoncent « non lu »/« non joué » ; ce comportement peut être désactivé dans les Options.
Corrections de bugs
• Correction de l’extraction de texte EPUB pour les livres contenant des commentaires HTML inline (<!-- ... -->) : le texte des chapitres est désormais correctement analysé au lieu d’être partiellement ou totalement ignoré.
• Correction du dictionnaire Wiktionary en espagnol et de la gestion du cache : des mots comme « agua » sont maintenant trouvés correctement et les anciennes entrées « mot introuvable » ne sont plus réutilisées.
• Correction de l’encodage lors de l’import d’articles RSS pour certaines sources espagnoles (ex. El Mundo) : les accents et le « ñ » sont désormais correctement conservés dans l’éditeur temporaire.
• Correction du décodage ANSI des fichiers d’Europe centrale (ex. tchèque/polonais) : Sonarpad distingue désormais mieux UTF-8 et ANSI et choisit la bonne page de codes (y compris Windows-1250), évitant les diacritiques corrompus.
• Correction de la persistance des sources RSS avec paramètres d’URL (ex. `rss.aspx?c=...`) : ces flux sont maintenant correctement sauvegardés et restaurés après redémarrage de Sonarpad.
• Correction de l’ouverture des fichiers pointeurs Google Drive (`.gdoc`, `.gsheet`, `.gslides`) depuis le menu contextuel de l’Explorateur : si la lecture directe échoue avec « Incorrect function (os error 1) », Sonarpad utilise désormais un fallback shell-open et le document s’ouvre correctement.
• Correction de la lecture des fichiers Excel legacy `.xls` (Excel 2010) : les anciens fichiers binaires sont maintenant détectés/décodés correctement au lieu d’afficher du texte corrompu (ex. `ÐÏ_à¡±...`).
• Correction du flux d’annonce du correcteur orthographique : les fautes sont désormais réannoncées lors d’une relecture ultérieure du texte, et la même faute est de nouveau signalée si elle est supprimée puis retapée.
• Correction des opérations de texte par ligne (ex. Ctrl+Q / Ctrl+Shift+Q, trier/inverser/lignes uniques/fusionner les lignes) : en sélectionnant une seule ligne avec Maj+Flèche bas, les lignes adjacentes ne sont plus fusionnées ni tronquées.
• Correction du comportement multilignes des opérations de texte par ligne (Ctrl+Q / Ctrl+Shift+Q et outils associés) : lorsque RichEdit fournit des séparateurs de ligne en CR seul, ils sont désormais normalisés correctement et toutes les lignes sélectionnées sont traitées sans couper le premier caractère.
• Extension de la normalisation d’entrée TTS pour les symboles visibles d’espace/tabulation/saut de ligne (␠/U+2420, ␣/U+2423, ␉/U+2409, ␊/U+240A, ␍/U+240D, ␤/U+2424), qui pouvaient provoquer des répétitions de paragraphes avec les voix multilingues.
• Affinement de la sanitisation du texte Edge TTS avec une pipeline unique de validation : normalisation des espaces étranges/invisibles, compactage des longues séquences de ponctuation (comme "...", "!!!", "???") et suppression des segments composés uniquement de ponctuation pour éviter les boucles de lecture.
• Correction de l’annonce du temps de lecture (Ctrl+I) pour les flux MP3/podcast : le temps courant est désormais borné à la durée de la piste, et la lecture est arrêtée automatiquement si la position dépasse la fin.
• Amélioration de la couverture de localisation de l’installateur : setup.exe inclut désormais aussi le tchèque, le polonais, le français et le serbe, tandis que le MSI reste un paquet unique en-US pour éviter la confusion en release.
• Correction du nettoyage à la désinstallation des entrées du menu contextuel : « Ouvrir avec Sonarpad » est maintenant supprimé de façon fiable, y compris dans des scénarios de registre legacy.
• Correction de la fiabilité pause/reprise en SAPI5 : la pause avec F4 fonctionne désormais correctement et la reprise revient au point attendu au lieu de redémarrer depuis le début.
• Correction du flux pause + recherche + reprise en lecture média : après une pause puis un déplacement avec Gauche/Droite, la touche Espace reprend désormais de manière fiable à la position courante au lieu de s'arrêter ou de repartir du début.

Version 0.6.5 – 2026-02-07
Améliorations
• Traduction espagnole améliorée grâce à Arturo Fernandez Rivas.
• Les imports RSS utilisent désormais un onglet temporaire dédié (titre localisé) ; Enregistrer sous le convertit en document normal.
• Les messages du lecteur d’écran sont désormais également envoyés à JAWS lorsqu’il est disponible.
Corrections de bugs
• La lecture depuis le curseur (F5) démarre exactement au niveau du curseur. Avant, elle pouvait commencer quelques lignes au-dessus car l’offset du curseur ne correspondait pas aux positions CRLF/UTF-16.
• Correction d’un problème de redessin : en tapant sur une sélection, le texte précédent pouvait disparaître jusqu’au déplacement de la sélection.
• Correction du parsing des chapitres EPUB : les pages de couverture ou uniquement images ne génèrent plus de lecture de CSS (ex. « padding ») ni de titres « Sconosciuto ».
• Correction d’un échec lors du découpage par durée des EPUB : Edge TTS pouvait échouer avec des blocs vides ou trop longs ("Edge audio not sent").
• La fenêtre d’enregistrement de podcast est maintenant indépendante : vous pouvez utiliser l’éditeur pendant l’enregistrement.
• Les articles RSS décodent désormais les entités HTML (par ex. &quot;, &amp;, &lt;, &gt;).
• Enregistrer/Enregistrer sous propose désormais le nom du fichier existant lors de l’enregistrement de formats non réécrivables (ex. EPUB), au lieu de la première ligne.
• Correction d’un problème où les podcasts avec de nouveaux épisodes n’étaient pas annoncés comme non joués, et renommage de « Non écouté » en « Non joué » pour un libellé plus professionnel.

Version 0.6.4 – 2026-02-05
Améliorations
• Le programme a été renommé en Sonarpad pour mettre davantage l'accent sur le son et l'audio, qui sont la clé de ce programme.
• Ajout de la sélection des pistes audio dans le menu Lecture pour les fichiers multimédias avec plusieurs pistes audio (ex. MKV avec plusieurs langues).
• Les podcasts indiquent maintenant clairement ceux non écoutés avec le préfixe « Non écouté » avant le nom.
• Nouveau système de balises pour changer la voix dans le texte. Exemples :
  - Voix Microsoft (Edge) : <voice edge it-IT-IsabellaNeural>Bonjour</voice>
  - Voix SAPI5 : <voice sapi5 Microsoft Helena Desktop>Bonjour</voice>
  - Voix SAPI4 : <voice sapi4 #1>Bonjour</voice>
  - Avec vitesse/tonalité/volume : <voice edge it-IT-ElsaNeural speed=-20 pitch=-5 volume=-10>Bonjour</voice>
• Catégories de podcasts enrichies.
• Ajout d’une option dans le menu contextuel pour créer un livre audio à partir de la sélection.
• Ajout du découpage des livres audio par durée, avec la possibilité de choisir le nom du premier fichier.
• Libellé de l’auteur localisé dans la lecture des articles (ex. « par », « by », « di »).
• Ajout d’options d’indentation (tabulations/espaces avec largeur) et de Tab/Maj+Tab pour indenter/désindenter les lignes sélectionnées.
• Correction du nettoyage Markdown : gestion des puces « * » lorsque la conservation des listes est désactivée.
Corrections de bugs
• Corrigé un bug où les livres audio SAPI4 pouvaient être créés différemment de ce qui était attendu.
• Fenêtre Rechercher dans les fichiers : Entrée sur un résultat ouvre maintenant à la position correcte de l’extrait et Échap retourne aux résultats.
• Fenêtre Options : ajustement du layout visuel des onglets Général, Voix, Éditeur et Audio pour éviter des contrôles manquants ou coupés.
• Correction d’un problème de signets lors du changement de vitesse de lecture.
• Correction d’un problème avec Podcast Index et les catégories qui ne s’affichaient pas correctement.
• Correction du problème de l’apostrophe qui coupait la lecture : plus de lecture séparée pour les dialogues, utilisation des balises de voix.

Version 0.6.3 – 2026-01-30
Améliorations
• Amélioration de la détection du microphone.
• Ajout de la lecture instantanée pour tous les formats.
Corrections de bugs
• Correction du plantage dans la fenêtre des catégories de podcasts.

Version 0.6.2 – 2026-01-30
Nouvelles fonctionnalités
• Ajout de la prise en charge de l'exécution de fichiers (Shift+F5). Les utilisateurs peuvent sélectionner un interpréteur (par exemple, python) dans les Options, le rechercher sur l'ordinateur, et appuyer sur Shift+F5 exécute le script actuel. Les fichiers HTML s'ouvrent dans le navigateur.
• Ajout de la prise en charge des fichiers pointeurs Google Docs (.gdoc, .gsheet, .gslides), qui s'ouvrent automatiquement dans le navigateur par défaut.
• Ajout de la prise en charge du format de livre audio M4B (Apple/AAC).
• Ajout de l'option "Afficher les épisodes" dans le menu contextuel des résultats de recherche de podcasts pour parcourir et lire des épisodes sans s'abonner.
• Ajout de la fonctionnalité "Aller à la ligne" (menu Édition ou Ctrl+J) pour accéder rapidement à un numéro de ligne spécifique.
• Ajout d'options de menu contextuel pour ordonner les flux RSS et les podcasts (alphabétiquement ou par date).
• Ajout de flux RSS vietnamiens par défaut.
• Ajout d'une case de test du microphone dans la boîte de dialogue d'enregistrement pour vérifier les niveaux avant de commencer.
• Ajout de "Afficher la description" pour les épisodes de podcast dans le menu contextuel.
• Ajout de la prise en charge des formats audio/vidéo étendus via FFmpeg : mkv, avi, mov, m4v, webm, mpg, ts, wmv, flv, vob, 3gp, flac, ogg, wma, aiff.
• Ajout de la prise en charge de la lecture synchronisée des sous-titres (srt, vtt, ass, sub, sbv, lrc, smi) avec NVDA ou la voix sélectionnée. Le programme recherche un fichier de sous-titres portant le même nom que le fichier multimédia. Ajout des options "Importer des sous-titres" et "Supprimer les sous-titres" dans le menu Lecture pour les fichiers aux noms différents.
• Ajout d'associations de fichiers pour tous les nouveaux formats audio/vidéo pris en charge dans le menu contextuel "Ouvrir avec Sonarpad".
• Ajout d'un paramètre de réglage de la hauteur tonale pour n'importe quel fichier.
• Ajout d'une option dans les Paramètres généraux pour activer ou désactiver les rapports d'erreurs anonymes. Ajout d'une entrée dans le menu Aide pour créer un fichier ZIP de diagnostic.
• Ajout d'une option pour utiliser une voix différente pour les dialogues, à la fois pour la lecture en direct et la création de livres audio.
• Ajout d'un navigateur de catégories de podcasts pour explorer les podcasts par catégorie (affaires, art, sport, etc.).
Améliorations
• L'ouverture d'un fichier audio/vidéo depuis l'Explorateur ouvre désormais directement la vue du lecteur au lieu de l'éditeur de texte.
• Suppression de l'invite OCR pour les PDF inaccessibles ; l'OCR est désormais effectué automatiquement pour améliorer la vitesse et l'expérience utilisateur.
• Amélioration du terminal accessible : la lecture NVDA mémorise désormais la dernière ligne lue pour une meilleure continuité.
• SAPI 4 : La création de livres audio est désormais entièrement parallélisée et presque instantanée. Ajout d'une invite pour choisir le nombre de processus simultanés.
• SAPI 4 : Élimination du goulot d'étranglement de la conversion WAV vers MP3 en convertissant les morceaux en parallèle pendant la synthèse.
• SAPI 4 : Amélioration de la gestion des erreurs et du nettoyage automatique des fichiers temporaires.
• Boîte de dialogue Rechercher : Renommage de "Regex" en "Expression régulière" pour plus de clarté et ajout des traductions manquantes pour les options de recherche.
• Livres audio M4B : Meilleure gestion de la sortie ; la division par parties/marqueurs produit désormais un seul fichier M4B avec des métadonnées de chapitres incluant le titre et l'auteur.
• Lecteur : Correction de la précision des signets et de l'annonce du temps lorsque la vitesse de lecture n'est pas de 1.0x.
• Restauration de la navigation Ctrl+Tab et Ctrl+Maj+Tab dans les Options.
• Ajout d'une option dans le menu Lecture pour réinitialiser instantanément la vitesse à la normale (1.0x).
• Mise à jour de toutes les dépendances vers les dernières versions pour de meilleures performances et stabilité.
• Intégration de FFmpeg avec chargement dynamique de DLL pour assurer la compatibilité sans bloquer le démarrage.
• Mise à jour des filtres de téléchargement de podcasts pour inclure les nouveaux formats audio/vidéo.
• Empêchement de Ctrl+S d'enregistrer les fichiers audio/vidéo pour éviter la corruption.
• Amélioration de l'importation des transcriptions YouTube, la rendant plus robuste et résiliente.
• Amélioration de la robustesse de la division des livres audio en parties, garantissant qu'aucun texte n'est perdu.
• L'installateur est désormais entièrement multilingue, prenant en charge l'italien, l'anglais, l'espagnol, le portugais, le suédois et le vietnamien en fonction de la langue du système de l'utilisateur. L'anglais est la valeur par défaut pour les systèmes non pris en charge.
• Catégories de podcasts : appuyer sur Entrée sur une catégorie confirme désormais la sélection (équivalent au bouton OK).
• Amélioration du système de détection des blocages pour éviter les faux positifs lorsque des boîtes de dialogue modales sont ouvertes (messages d'erreur, "texte non trouvé").
Corrections
• Correction d'un bug où le journal des modifications ne s'ouvrait pas au démarrage.
• Correction d'un bug où l'invite OCR n'apparaissait pas pour les PDF inaccessibles ouverts depuis l'Explorateur.
• Correction d'un bug au démarrage pouvant entraîner une perte de focus ou la fermeture de la fenêtre immédiatement après l'ouverture.
• Correction d'un bug critique dans la recherche par expression régulière empêchant de trouver du texte, y compris des problèmes avec la "Recherche circulaire" et l'option "Le point équivaut à une nouvelle ligne" avec les fins de ligne Windows.
Localisation
• Ajout de la traduction en polonais.
• Ajout de la traduction en français.
• Ajout de la traduction en tchèque (merci à Radek Žalud et Jiri Holzinger).

Version 0.6.1 – 2026-01-20
Corrections
• Correction d'un bug où l'activation de "Afficher les voix dans l'éditeur" provoquait l'arrêt de la lecture du podcast.
• Correction d'un problème où certains podcasts ne pouvaient pas être ajoutés via URL car l'URL était tronquée.
• Correction d'un bug où les URL normales ne pouvaient plus être ajoutées dans la fonctionnalité de flux RSS.
• Correction d'un problème où l'option de langue de Wikipédia était affichée plusieurs fois dans différents onglets de paramètres.
• Suppression de la création de fichiers de débogage qui étaient générés incorrectement même en mode release.
Améliorations
• Amélioration de la prise en charge des voix Microsoft, qui utilisent désormais une méthode de lecture dédiée avec un agent utilisateur différent.
• Ajout de la prise en charge des fichiers MP4.

Version 0.6.0 – 2026-01-20
Nouvelles fonctionnalités
• Ajout du correcteur orthographique. Depuis le menu contextuel, les utilisateurs peuvent vérifier si le mot actuel est correct et, sinon, obtenir des suggestions d'orthographe.
• Ajout de l'importation et de l'exportation de podcasts via des fichiers OPML.
• Ajout de la prise en charge de la recherche Podcast Index en plus d'iTunes. Les utilisateurs peuvent saisir leur clé API et leur secret gratuits (générés uniquement à l'aide d'une adresse e-mail).
• Ajout de la prise en charge des voix SAPI4, tant pour la lecture en temps réel que pour la création de livres audio.
• Ajout du repli automatique OCR pour les PDF non accessibles : lorsqu'aucun texte extractible n'est trouvé, le document est reconnu via OCR.
• Ajout de la prise en charge du dictionnaire utilisant le Wiktionnaire. Appuyer sur la touche Applications affiche les définitions et, lorsqu'ils sont disponibles, les synonymes et les traductions dans d'autres langues.
• Ajout de l'importation d'articles Wikipédia avec recherche, sélection de résultats et importation directe dans l'éditeur.
• Ajout du raccourci Maj+Entrée dans le module RSS pour ouvrir un article directement sur le site web d'origine.
Améliorations
• La sélection du microphone est désormais toujours respectée par l'application.
• Dans la fenêtre des podcasts, appuyer sur Entrée sur un épisode annonce désormais immédiatement "chargement" via NVDA pour confirmer l'action.
• Dans les résultats de recherche de podcasts, appuyer sur Entrée s'abonne désormais au podcast sélectionné.
• Correction et amélioration des étiquettes pour les raccourcis Ctrl+Maj+O et Podcast Ctrl+Maj+P.
• La vitesse et le volume de lecture sont désormais enregistrés dans les paramètres et persistent pour tous les fichiers audio.
• Ajout d'un dossier cache dédié pour les épisodes de podcast. Les utilisateurs peuvent conserver les épisodes via "Garder le podcast" dans le menu Lecture. Le cache est automatiquement nettoyé lorsqu'il dépasse la taille définie par l'utilisateur (Options → Audio).
• Amélioration significative de la récupération des articles RSS en utilisant l'emprunt d'identité libcurl avec des profils Chrome et iPhone, assurant une compatibilité avec ~99% des sites.
• Ajout de l'état lu / non lu pour les articles RSS, avec une indication claire dans la liste RSS.
• Tout remplacer signale désormais le nombre de remplacements effectués.
• Ajout d'un bouton Supprimer le podcast lors de la navigation dans la bibliothèque de podcasts à l'aide de Tab.
Corrections
• Suppression de l'entrée redondante "mise à jour en attente" du menu Aide (les mises à jour sont déjà gérées automatiquement).
• Correction d'un bug où appuyer sur Ctrl+S sur un fichier MP3 ouvert enregistrait et corrompait le fichier.
• Correction d'un problème d'interface utilisateur où "Audiolivres par lots" était affiché comme "(B)… Ctrl+Maj+B" (suppression de l'étiquette redondante).
• Correction des guillemets intelligents : lorsqu'ils sont activés, les guillemets normaux sont désormais correctement remplacés par des guillemets intelligents.
• Correction d'un bug où l'utilisation de "Aller au signet" réinitialisait la vitesse de lecture à 1.0.
• Correction d'un problème où les épisodes de podcast déjà téléchargés étaient retéléchargés au lieu d'utiliser la version en cache.
Raccourcis clavier
• F1 ouvre désormais le Guide d'aide.
• F2 vérifie désormais les mises à jour.
• F7 / F8 sautent désormais à l'erreur d'orthographe précédente ou suivante.
• F9 / F10 basculent désormais rapidement entre les voix favorites.
Améliorations développeur
• Les erreurs ne sont plus ignorées silencieusement : tous les modèles let _ = ont été supprimés, et les erreurs sont désormais gérées explicitement.
• Le projet ne compile plus s'il y a des avertissements.
• Les implémentations personnalisées telles que les aides de style strlen / wcslen ont été supprimées.
• La gestion des DLL a été nettoyée et consolidée autour de libloading.
• Les aides d'analyse d'octets manuelles ont été supprimées au profit des méthodes standard.

Version 0.5.9 - 2026-01-13
Nouvelles fonctionnalités
• Ajout de la réorganisation RSS depuis le menu contextuel (monter/descendre/vers la position) avec vérification de position invalide.
• Ajout d'un menu contextuel d'article avec ouverture du site d'origine et partage via WhatsApp, Facebook et X.
• Ajout du raccourci Échap pour revenir des articles importés à la liste RSS.
• Ajout du mode podcast : recherche, abonnement, écoute ; réorganisation des abonnements ; Échap arrête la lecture et revient à la liste ; Entrée sur un épisode démarre la lecture.
• Ajout du contrôle de la vitesse de lecture pour les podcasts et les fichiers MP3.
• Ajout de Ctrl+T pour sauter à un temps spécifique.
• Ajout d'un bouton d'aperçu vocal après le combo de volume.
• Ajout de la recherche et du remplacement par regex (style Notepad++).
• Ajout de l'importation RSS depuis des fichiers OPML et TXT.
• Ajout d'une option pour activer "Ouvrir avec Sonarpad" dans l'Explorateur de fichiers, y compris pour les versions portables.
Améliorations
• Amélioration de la sélection de la vitesse/hauteur/volume de la voix, respectant les limites maximales TTS.
• Diverses améliorations RSS pour télécharger tous les articles sans déplacer le focus NVDA pendant les mises à jour.
• Amélioration de la lecture audio avec un menu dédié, annonce du temps Ctrl+I, et volume jusqu'à 300%.
• Ajout de raccourcis manquants pour certaines fonctions.
• Réorganisation du menu Édition avec un sous-menu de nettoyage de texte.
• Réorganisation des Options en onglets, avec navigation Ctrl+Tab et Ctrl+Maj+Tab.
• Le lecteur RSS télécharge désormais le contenu complet de l'article, correspondant à la vue du navigateur.
Corrections
• Correction du nettoyage Markdown supprimant les numéros au début des lignes.
• Correction de AltGr+Z déclenchant l'annulation.
• Correction de l'annulation de l'enregistrement de livre audio pour qu'il s'arrête rapidement.
Localisation
• Ajout de la traduction vietnamienne (merci à Anh Đức Nguyễn).

Version 0.5.8 - 2026-01-10
Nouvelles fonctionnalités
• Ajout du contrôle du volume pour le microphone et l'audio système lors de l'enregistrement de podcasts.
• Ajout d'une nouvelle fonctionnalité pour importer des articles depuis des sites web ou des flux RSS, y compris les flux les plus importants pour chaque langue.
• Ajout d'une fonction pour supprimer tous les signets du fichier actuel.
• Ajout d'une fonction pour supprimer les lignes dupliquées et les lignes consécutives dupliquées.
• Ajout d'une fonction pour fermer tous les onglets ou fenêtres sauf l'actuel.
• Ajout d'une entrée Dons dans le menu Aide pour toutes les langues.
Améliorations
• Amélioration du terminal accessible pour éviter certains plantages.
• Amélioration et correction des touches d'accès et des raccourcis clavier dans toute l'application.
• Correction d'un problème où la fermeture de la fenêtre de lecture audio n'arrêtait pas la lecture.
• Ajout de boîtes de dialogue de confirmation pour les actions importantes (ex: supprimer les lignes dupliquées, supprimer les traits d'union de fin de ligne, supprimer tous les signets).
• Ajout de la possibilité de supprimer des flux/sites RSS de la bibliothèque en les sélectionnant et en appuyant sur Suppr.
• Ajout d'un menu contextuel dans la fenêtre RSS pour modifier ou supprimer des flux/sites RSS.
• Suppression du paramètre pour déplacer les paramètres vers le dossier actuel ; l'application gère désormais cela automatiquement en fonction de l'emplacement.

Version 0.5.7 - 2026-01-05
Nouvelles fonctionnalités
• Ajout de la fonctionnalité Audiolivres par lots pour convertir plusieurs fichiers/dossiers à la fois.
• Ajout de la prise en charge des fichiers Markdown (.md).
• Ajout de la sélection de l'encodage de fichier lors de l'ouverture de fichiers texte.
• Ajout d'une option dans le terminal accessible pour annoncer les nouvelles lignes avec NVDA.
Améliorations
• L'enregistrement de livre audio sauvegarde désormais nativement en MP3 lorsqu'il est sélectionné.
• L'utilisateur peut désormais choisir la position de l'astérisque (*) "modifications non enregistrées" dans le titre de la fenêtre.
• Amélioration de la robustesse du système de mise à jour.
• Ajout de "Supprimer les traits d'union" dans le menu Édition pour corriger les fins de ligne OCR.

Version 0.5.6 - 2026-01-04
Corrections
  Amélioration de la recherche dans les fichiers pour que l'appui sur Entrée ouvre le fichier exactement à l'extrait sélectionné.
Améliorations
  Ajout de la prise en charge PPT/PPTX (ouvrir comme texte).
  L'ouverture de formats non textuels enregistre désormais en .txt pour éviter la corruption de formatage (PDF/DOC/DOCX/EPUB/HTML/PPT/PPTX).
  Ajout de l'enregistrement de podcast à partir du microphone et de l'audio système (menu Fichier, Ctrl+Maj+R).

Version 0.5.5 – 2026-01-03
Nouvelles fonctionnalités
• Ajout d'un terminal accessible optimisé pour les grandes sorties et les lecteurs d'écran (Ctrl+Maj+P).
• Ajout d'un paramètre pour enregistrer les paramètres utilisateur dans le dossier actuel (mode portable).
Corrections
• Amélioration des extraits de recherche dans les fichiers pour que l'aperçu reste aligné avec la correspondance.

Version 0.5.4 – 2026-01-03
Améliorations
• Correction de la normalisation des espaces (Ctrl+Maj+Entrée).
• Ajout de la prise en charge HTML/HTM (ouvrir comme texte).

Version 0.5.3 – 2026-01-02
Nouvelles fonctionnalités
• Ajout de la recherche dans les fichiers.
• Ajout de nouveaux outils de texte : Normaliser les espaces, Saut de ligne dur et Supprimer Markdown.
• Ajout des statistiques de texte (Alt+Y).
• Ajout de nouvelles commandes de liste dans le menu Édition :
• Ordonner les éléments (Alt+Maj+O)
• Garder les éléments uniques (Alt+Maj+K)
• Inverser les éléments (Alt+Maj+Z)
• Ajout de Citer / Retirer la citation des lignes (Ctrl+Q / Ctrl+Maj+Q).
Localisation
• Ajout de la localisation espagnole.
• Ajout de la localisation portugaise.
Améliorations
• Lorsqu'un fichier EPUB est ouvert, Enregistrer bascule désormais automatiquement vers Enregistrer sous et exporte le contenu en fichier .txt pour éviter la corruption de l'EPUB.

## 0.5.2 - 2026-01-01
- Ajout d'un journal des modifications.
- Ajout des options ouvrir avec Sonarpad et des associations de fichiers lors de l'installation.
- Amélioration de la localisation des messages.
- Ajout de la sélection de partie lors de l'utilisation de "Diviser le livre audio par texte".
- Ajout de l'importation de transcription YouTube.

## 0.5.1 - 2025-12-31
- Mises à jour automatiques avec confirmation.
- Améliorations de l'exportation de livres audio.
- Améliorations TTS.
- Menu Affichage et panneaux voix/favoris.
- Langue par défaut du système et améliorations de la localisation.
- CI et empaquetage Windows.

## 0.5.0 - 2025-12-27
- Refactorisation modulaire.
- Flux de travail de construction/empaquetage Windows.
- Correction de la navigation par TAB dans la fenêtre d'aide.

## 0.5 - 2025-12-27
- Changement de version préliminaire.

## 0.1.0 - 2025-12-25
- Version initiale.

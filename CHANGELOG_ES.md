# Changelog

Versión 0.9.5 – 2026-09-07

Audiodescripción con IA
1. Mejorada la resistencia cuando Gemini devuelve temporalmente `MALFORMED_RESPONSE` sin contenido utilizable. Sonarpad ahora vuelve a intentar exactamente el mismo fragmento hasta tres veces, con una breve espera entre intentos, en lugar de detener inmediatamente toda la audiodescripción. Las respuestas normales de Gemini, los bloqueos de seguridad, los errores de límite de tokens y el comportamiento actual de los puntos de control permanecen sin cambios.

2. Corregido otro caso poco frecuente de `invalid Gemini chunk timeline` con algunos vídeos cuyos fragmentos preparados por FFmpeg informan de una pequeña desviación acumulada de duración. Sonarpad mantiene sin cambios la ruta normal cuando la línea de tiempo es válida y utiliza un reajuste conservador solo cuando la línea de tiempo normal fallaría y la diferencia total es pequeña; las discrepancias grandes o sospechosas siguen produciendo un error.

3. Añadido el servicio opcional Sonarpad AI para las audiodescripciones con IA. Los usuarios pueden seguir usando su propia clave API de Gemini o elegir el servicio Sonarpad e introducir un código Sonarpad personal. El código se protege con Windows DPAPI. En el modo de servicio, los fragmentos de vídeo preparados se cargan directamente desde el PC del usuario a Google mediante una URL temporal de carga de Google; sonarpad.com no recibe ni almacena los bytes del vídeo. El backend solo autoriza el acceso, impone el modelo Gemini/Standard y los límites de uso, contabiliza el consumo y devuelve la respuesta de Gemini.

4. Mejorada la edición de proyectos de audiodescripción guardados. Ahora se pueden modificar varias descripciones seguidas sin tener que aplicar cada cambio inmediatamente. Los cambios permanecen en memoria; al pulsar “Aplicar texto”, Sonarpad comprueba cada descripción modificada con su intervalo guardado y después guarda juntas todas las modificaciones válidas. Si una descripción no cabe en su intervalo, el foco vuelve a ella sin perder los demás cambios pendientes.

5. Añadido “Ajustar voz” inmediatamente antes de “Crear audiodescripción”. La nueva ventana permite elegir motor, voz, velocidad y volumen e incluye “Probar voz”. Al pulsar OK, el foco vuelve exactamente a “Ajustar voz” y los valores elegidos se aplican a la audiodescripción que se va a crear. Si no se usa esta ventana, la síntesis permanece exactamente como antes.


6. Se ampliaron los controles de voz en los proyectos de audiodescripción guardados. En Modificar proyecto, además del motor y la voz, ahora están disponibles la velocidad, el volumen y “Probar voz”. Al aplicar los parámetros de voz, Sonarpad vuelve a comprobar todas las descripciones existentes con los nuevos parámetros de síntesis y reconstruye el MP3 solo si todas siguen entrando en sus intervalos guardados. Se reutilizan los intervalos sin diálogo, los tiempos de Visual Evidence, las descripciones de Gemini y el análisis temporal del proyecto sin volver a ejecutar el análisis de IA.

Versión 0.9.4 – 2026-09-04

Audiodescripción con IA
1. Corregido un problema con vídeos Matroska/WebM que contienen una marca de tiempo inicial interna inusualmente alta y podían detener la generación de la audiodescripción con el error “invalid Gemini chunk timeline”. Sonarpad ahora normaliza la duración del archivo de origen y de los fragmentos de Gemini solo cuando detecta este patrón anómalo de marcas de tiempo, dejando sin cambios los vídeos normales y el comportamiento de audiodescripción que ya funcionaba.
2. Mejorado el ducking de la audiodescripción para obtener una mezcla más natural. El audio original ahora baja suavemente antes de que empiece la voz y vuelve de forma gradual a su nivel normal después de la descripción; además, las descripciones muy próximas mantienen un nivel de fondo estable, evitando continuas bajadas y subidas.
3. Corregido un problema que podía hacer fallar la exportación final de la audiodescripción con algunos archivos AVI y otras fuentes con audio 5.1/multicanal cuando la decodificación terminaba con un fotograma de audio intercalado incompleto. Sonarpad ahora completa de forma segura únicamente ese último fotograma parcial con las muestras de silencio estrictamente necesarias; los fotogramas ya completos y los archivos mono, estéreo o multicanal normales permanecen sin cambios.


Versión 0.9.3 – 2026-09-03

Voces SAPI5
1. Corregido un problema por el que algunas voces SAPI5 locales podían no hablar con el movimiento del cursor activado, durante la lectura con varias voces o al crear audiolibros/MP3. Sonarpad usa ahora una ruta de síntesis SAPI5 a archivo que funciona de forma fiable en Windows y que sigue permitiendo cancelar la síntesis, mientras que la reproducción directa normal permanece sin cambios.
2. Corregida la posición del cursor durante la lectura de diálogos con varias voces. Las etiquetas de voz insertadas automáticamente por Sonarpad para los diálogos ahora se tratan solo como metadatos de reproducción y ya no como caracteres del editor, evitando que tras F4 o F6 el cursor salte por delante del texto real. La lectura con una sola voz y las etiquetas <voice> escritas explícitamente en los documentos no cambian.

Audiodescripción con IA
1. Añadida la casilla “Mostrar clave API” inmediatamente después del campo de la clave API de Gemini. Está desactivada de forma predeterminada; al activarla muestra temporalmente la clave completa para comprobar que se haya pegado entera. Al volver a abrir la ventana, la clave aparece siempre oculta.

Pódcasts y Wikipedia
1. Después de guardar una grabación de pódcast, Sonarpad ahora pregunta si se desea abrir la carpeta que contiene el archivo guardado, igual que al guardar contenido de YouTube/streaming.
2. Al importar otro artículo de Wikipedia en un editor que ya contiene texto, el nuevo artículo ahora se añade al final en lugar de insertarse al principio. El cursor se coloca al comienzo del artículo recién importado.


Versión 0.9.2 – 2026-09-02

Audiodescripción con IA
1. Corregido un problema que podía hacer fallar la audiodescripción con IA durante la exportación final a MP3 en vídeos con audio multicanal, como 5.1. Sonarpad ahora convierte automáticamente el audio multicanal a estéreo solo cuando es necesario para codificar MP3, sin cambiar las exportaciones mono o estéreo.
2. Al iniciar Crear audiodescripción con IA con un vídeo que contiene varias pistas de audio, Sonarpad ahora pregunta qué pista se debe utilizar antes de empezar. El cuadro combinado accesible se puede cambiar con las flechas; Aceptar inicia la audiodescripción con la pista seleccionada, mientras que Cancelar cierra la ventana de audiodescripción y devuelve el foco al editor de Sonarpad.

YouTube y streaming
1. Corregido un problema por el que, al iniciar Crear audiodescripción con IA desde un vídeo situado en la página 2 o posteriores de una lista de reproducción o canal de YouTube, podía volver a abrirse la ventana de selección de YouTube y quitar el foco a la ventana de audiodescripción. Sonarpad ahora cierra correctamente el selector sin volver a las páginas anteriores.
Versión 0.9.1 – 2026-09-01

Descargas de YouTube
• Corregido un problema por el que las ventanas de progreso de las descargas de YouTube/streaming podían volver repetidamente al primer plano después de cambiar a otra aplicación con Alt+Tab. Las descargas ahora continúan en segundo plano sin robar el foco.
• Mejorada la accesibilidad del progreso de las descargas. Al volver a la ventana de progreso, los lectores de pantalla pueden leer el estado actual y el porcentaje. En las listas de reproducción, Sonarpad también indica el número del elemento actual, el total de elementos y el título.
• Corregidos falsos avisos de bloqueo del watchdog durante descargas y conversiones largas cuando la ventana de progreso seguía respondiendo.
• Se añadió un cuadro combinado Formato a las descargas de listas de reproducción. Desde la lista de vídeos, pulse Tab para elegir MP4, MP3, M4A, OPUS, OGG, WAV o FLAC antes de iniciar la descarga múltiple.
• Se reorganizó el guardado de medios en streaming. El formato y la calidad ahora se eligen al guardar, en lugar de hacerlo en la ventana inicial de búsqueda de streaming. “Guardar multimedia” abre un único diálogo de Formato y Calidad, y las descargas de listas de reproducción incluyen ambos cuadros combinados.

Audiodescripción con IA
• Se corrigió un problema que podía impedir el inicio de la audiodescripción con IA con algunos vídeos MKV. Sonarpad ahora gestiona de forma más fiable los vídeos con marcas de tiempo irregulares o ausentes.

Versión 0.9.0 – 2026-08-31

Audiodescripción con IA — nueva función principal
• Se añadió “Crear audiodescripción con IA” en Herramientas > Multimedia. Sonarpad analiza el audio para encontrar espacios sin diálogo, genera las descripciones con Gemini y utiliza los motores de voz ya disponibles, evitando hablar sobre los diálogos.
• Se mejoró la sincronización entre lo que ocurre en el vídeo y las descripciones, con controles automáticos sobre los tiempos generados por Gemini.
• “Activar pausas extendidas” está desmarcada de forma predeterminada. Puede activarse en contenidos con mucho diálogo o poco espacio disponible para permitir descripciones más largas.
• Sonarpad puede intentar reconocer a los personajes y usar sus nombres. Los catálogos de personajes pueden mantenerse entre episodios de una serie para mejorar la continuidad.
• Es posible guardar el proyecto, modificar después las descripciones y volver a exportar sin tener que regenerarlo todo con Gemini.
• Si el proceso se interrumpe, Sonarpad conserva el progreso y permite continuar la audiodescripción. Si se agota la cuota de Gemini, se puede esperar, cambiar de modelo o detenerse sin perder el trabajo completado.
• La ventana permite elegir idioma, nivel de detalle, modelo Gemini, motor y voz, y recuerda las preferencias utilizadas.
• El módulo está disponible en los 17 idiomas de Sonarpad. Durante la generación la interfaz muestra solo el progreso, el estado actual y Cancelar; al terminar, el MP3 puede abrirse directamente en el reproductor interno.

Libros electrónicos y documentos
• Se añadió la importación de Kindle sin DRM en formatos MOBI, AZW y AZW3, con texto y capítulos disponibles en el editor y en el índice.
• Se añadió soporte para DAISY 2.02 y DAISY 3. Los audiolibros DAISY usan el reproductor interno de Sonarpad y respetan la navegación y los límites de los capítulos.
• Kindle y DAISY se importan sin sobrescribir el archivo original; los Kindle protegidos con DRM se rechazan explícitamente.
• Se corrigió “Guardar como” para EPUB: al elegir TXT u otro formato se usa ahora la extensión seleccionada y el EPUB original permanece asociado al documento abierto.

RSS y artículos
• Se añadió la selección múltiple de artículos RSS para eliminar varios en una sola operación.
• Los RSS admiten ahora carpetas reales que se conservan durante la importación y exportación OPML, incluidas las carpetas vacías.
• Los feeds pueden reordenarse dentro de la carpeta actual con Mover arriba, Mover abajo, Mover al principio, Mover al final y Mover a la posición.

Accesibilidad, guías e interfaz
• Las guías de Sonarpad se han reorganizado con un índice y se ha añadido una guía completa de Audiodescripción con IA.
• Se corrigió un problema de la traducción alemana que podía impedir que aparecieran Abrir, Guardar como y otros cuadros de selección de archivos.

Voces e idiomas
• El catálogo descargable de Google TTS pasa de 104 a 156 paquetes y de 53 a 81 variantes de idioma.
• Se añadieron nuevos paquetes de Google TTS y nombres localizados para más idiomas en toda la interfaz.

Versión 0.8.4 – 2026-07-24

Edición de documentos EPUB
• Sonarpad ahora no solo puede abrir documentos EPUB, sino también modificarlos y volver a guardarlos en formato EPUB, conservando el formato original, el índice, las notas al pie, las imágenes, las hojas de estilo, los metadatos y los enlaces internos.
• El formato EPUB está disponible en «Guardar como» para los documentos abiertos desde un EPUB. Al guardar, solo se actualiza el texto modificado y se mantiene intacta la estructura del libro.

Fiabilidad de los audiolibros
• Corregido un problema intermitente por el que, después de cinco intentos fallidos de Google TTS, una unidad de síntesis se descartaba silenciosamente y podía faltar parte del texto en el audiolibro final.
• Las unidades de Google se vuelven a intentar hasta que funcionan o hasta que el usuario cancela. El inicio de los procesos se escalona para reducir conflictos temporales con Chrome y los archivos; además, Sonarpad detiene la creación en lugar de guardar un audiolibro al que le falte un segmento.
• Los audiolibros con Edge ahora reintentan sin un límite fijo los errores temporales de red, WebSocket, tiempo de espera, limitación del servicio y audio no válido hasta que la síntesis finaliza o el usuario cancela, también con voces mixtas y división por tiempo. SAPI4 y SAPI5 conservan reintentos adaptativos y limitados; si un segmento sigue fallando, Sonarpad detiene el proceso sin guardar un audiolibro incompleto.

Navegación de las bibliotecas digitales
• Los resultados de LibriVox, Internet Archive y Project Gutenberg ahora usan una navegación por páginas como YouTube: «Ir a los resultados anteriores» aparece al principio de la lista e «Ir a los siguientes resultados» al final.
• Corregidas las transiciones del foco en LibriVox: al abrir un libro o un capítulo, el foco de NVDA ya no pasa al editor principal antes de abrirse la lista siguiente o el reproductor.
• Añadida una protección del foco durante las búsquedas y la carga de libros de LibriVox: una ventana de carga traducida permanece en primer plano durante toda la solicitud, evitando que el foco de NVDA pase al símbolo del sistema, Windows Terminal u otra aplicación.

Descarga de listas de reproducción de YouTube
• Añadido a las listas de reproducción de YouTube un comando accesible de selección múltiple que permite elegir qué vídeos descargar sin modificar el comando «Guardar multimedia» del elemento que se está reproduciendo.
• Los elementos seleccionados se descargan de uno en uno con el formato y la calidad elegidos al abrir la lista, reciben nombres numerados que conservan el orden original y se guardan en una carpeta propia dentro de la carpeta Multimedia configurada.
• La ventana incluye «Seleccionar todo» y «Deseleccionar todo», anuncia cuántos elementos están seleccionados, permite cancelar conservando los archivos ya terminados e informa claramente de los elementos que no pudieron descargarse.
• Los elementos de la lista de reproducción son ahora casillas de verificación nativas: los lectores de pantalla anuncian automáticamente el título, el tipo de control y el estado marcado o desmarcado, sin añadir palabras al título visible ni utilizar anuncios de voz forzados.

Versión 0.8.3 – 2026-07-23

Modo oscuro
• Añadido un modo oscuro que puede activarse desde el menú Ver y se guarda en las preferencias.
• El tema oscuro se aplica al editor, los menús, las ventanas secundarias y los controles principales, adaptando los colores del texto para mantener la legibilidad y la accesibilidad.

Idioma alemán
• Añadido el alemán como idioma completo de la interfaz, seleccionable desde Opciones.
• Noticias y RSS, corrector ortográfico, calendario y todas las citas, donaciones, guía y registro de cambios están disponibles íntegramente en alemán.

Portugués de Brasil y Google News
• Añadido el portugués de Brasil como idioma completo de la interfaz, separado del portugués de Portugal y seleccionable desde Opciones.
• La interfaz, el calendario y todas las citas, el corrector ortográfico, las donaciones, la guía y el registro de cambios están disponibles íntegramente en portugués de Brasil.
• Google News admite ahora la localización de Brasil, las categorías brasileñas y fuentes RSS brasileñas predeterminadas independientes.
• Cuando el canal las proporciona, las fuentes de Google News relacionadas con la misma noticia se muestran como elementos secundarios accesibles en el árbol.

LibriVox
• Se ha optimizado la búsqueda de LibriVox para evitar solicitudes excesivas al servicio y bloqueos de la interfaz. Se eliminaron los recorridos extensos del catálogo, se redujeron los intentos y se introdujeron tiempos de espera más breves.

Síntesis de voz
• Las secuencias de tres o más puntos ahora se normalizan antes de la lectura, evitando que algunas voces pronuncien «punto punto» o generen segmentos formados únicamente por signos de puntuación.

Artículos relacionados de Google Noticias
• Para cada noticia, cuando están disponibles, ahora se muestran artículos relacionados, es decir, otros artículos que tratan la misma noticia. Para leerlos, basta con expandir el artículo principal cuando Sonarpad indique que hay artículos relacionados disponibles. Quien no quiera expandir esta sección solo tiene que pulsar Intro sobre el artículo principal y leer la noticia como siempre.
• Los artículos relacionados ahora utilizan el mismo sistema de leído/no leído que los artículos principales, incluidos los anuncios accesibles, la fecha y la hora, el guardado del estado y su conservación tras actualizar las fuentes o reiniciar Sonarpad.

Anuncios en las partes de los audiolibros
• Se añadió a las opciones de audio el cuadro combinado «Anuncio al inicio de cada parte». En los audiolibros divididos en varios archivos, cada parte puede comenzar sin anuncio, con el título del libro, el título y el número de parte, el nombre del archivo o el nombre del archivo y el número de parte.

Versión 0.8.2 – 2026-07-17

Bibliotecas digitales y audiolibros
• Añadido Project Gutenberg, con búsqueda por título o autor y selección de idioma.
• Los libros EPUB de Project Gutenberg se descargan en Documentos\Sonarpad\Documents; al finalizar, Sonarpad pregunta si se desea abrir inmediatamente el libro en el editor.
• Añadido Internet Archive para buscar y escuchar colecciones de audio, incluidas emisiones de radio antiguas, discursos y música en directo.
• Añadido LibriVox para buscar audiolibros por título o autor y reproducir directamente sus capítulos con el mismo reproductor utilizado para los pódcast.
• Las tres nuevas funciones están disponibles en el menú Herramientas y, cuando está activada la agrupación de menús, en la sección Lectura.

Transcripciones de audio largas
• Corregida la transcripción de archivos de audio largos: el audio se divide ahora automáticamente en partes de 15 minutos, se transcribe una parte cada vez y después se vuelve a unir, evitando los errores que podían producirse con grabaciones de larga duración.

YouTube
• Las acciones más útiles que antes solo estaban disponibles después de abrir un vídeo de YouTube y acceder al menú Reproducción ahora también están disponibles directamente en el menú contextual del mismo vídeo, como «Transcribir audio actual», «Crear audiodescripción con IA» y «Guardar medio», para facilitar su uso.
• Añadida la opción «Copiar enlace», disponible también con Ctrl+C, para copiar al portapapeles la URL del vídeo, la lista de reproducción o el canal de YouTube seleccionado.

Versión 0.8.1 – 2026-07-16

Síntesis de voz de Google
• Corregido el inicio de Google TTS en sistemas Windows donde las conexiones aceptadas por el servidor interno del navegador heredaban el modo de socket no bloqueante, provocando el error 10035 e impidiendo que hablaran las voces descargadas.
• Sonarpad espera ahora a que el motor WASM de Chrome o Edge esté completamente cargado antes de la vista previa de voz o de la lectura con F5, evitando el error “Chrome WASM TTS engine was not loaded”.
• El navegador oculto desactiva la traducción de páginas y la accesibilidad del renderizador para evitar anuncios como “Traducir página” e interferencias con los comandos de lectura.
• El panel «Voces en el editor» muestra ahora el botón «Gestionar voces de Google...» cuando se selecciona el motor Google y actualiza inmediatamente la lista de voces instaladas al cerrar el gestor.
• Los avisos de dependencias mostrados al eliminar paquetes de voz de Google ahora están traducidos a todos los idiomas de la interfaz.

Experiencia de actualización
• Después de una actualización automática, la ventana de finalización con el registro de cambios se abre tras restaurar el foco inicial y permanece en primer plano, en lugar de aparecer solo al pulsar Tab.

Documentos PDF
• Corregidos los PDF cuyo texto incrustado contenía caracteres NUL y se truncaba en la primera aparición al cargarse en el editor.
• Si pdf-extract devuelve caracteres NUL incrustados, Sonarpad vuelve a intentarlo con PDFium; cualquier NUL restante se elimina antes de enviar el texto a los controles de Windows, conservando el resto del documento.

Accesibilidad de los menús
• Se eliminó el cálculo de mnemónicas durante la ejecución: las teclas de acceso están ahora escritas explícitamente en cada una de las 15 traducciones de la interfaz y permanecen idénticas en cada inicio.
• Se revisaron todas las entradas estables de los menús principales y submenús, incluidos Reproducción, las fuentes, Guardar imagen y Mostrar índice EPUB; las mnemónicas ausentes o duplicadas entre elementos del mismo nivel se corrigieron directamente en las traducciones.
• Las pruebas automáticas ahora solo validan las traducciones y fallan si falta una mnemónica, no es válida o está duplicada; ya no modifican las etiquetas durante la ejecución.
• En menús excepcionalmente grandes, cuando el texto traducido no ofrece suficientes caracteres distintos, se muestra una tecla de acceso numérica explícita con el formato estándar de Windows «(&1)».

Versión 0.8.0 – 2026-07-15

Diccionario en línea
• Se añadió el alemán al diccionario en línea Wiktionary.
• Las definiciones y los sinónimos en alemán ahora se reconocen correctamente según la estructura específica del Wiktionary alemán.

Fiabilidad de los audiolibros SAPI5
• La creación de audiolibros SAPI5 sigue utilizando hasta 12 trabajadores en paralelo cuando la voz seleccionada genera resultados fiables.
• Cada parte se comprueba mediante el tamaño del archivo, la duración estimada y una comparación prudente con el texto asignado.
• Las partes ausentes o sospechosas se regeneran automáticamente reduciendo progresivamente la concurrencia: 12, 8, 6, 4, 2 y finalmente 1 trabajador. Solo se repiten las partes problemáticas.
• El límite fiable se recuerda por separado para cada voz SAPI5, sin ralentizar las voces que funcionan correctamente con 12 trabajadores.
• Una comprobación final evita aceptar silenciosamente un MP3 mucho más corto que las partes generadas.
• Los detalles se guardan en `sapi5_audiobook_diagnostic.log`.
• Cada unidad de síntesis SAPI5 se ejecuta ahora en un proceso Sonarpad separado y oculto. Si una voz de terceros falla, solo se cierra ese trabajador y la aplicación principal permanece abierta.
• Durante la misma creación del audiolibro, las partes no completadas se reintentan inmediatamente con el siguiente nivel inferior de concurrencia; las partes ya validadas se conservan.
• La recuperación en el siguiente inicio permanece como protección adicional solo si se interrumpe la aplicación principal o el equipo.

Procesos de audiolibros SAPI4
• Ahora se respeta el número de procesos SAPI4 elegido por el usuario, hasta un máximo técnico de 64; se eliminó el límite oculto anterior de 16.
• El número efectivo solo se reduce cuando el audiolibro contiene menos unidades de trabajo que las solicitadas.
• Si uno o más procesos del puente SAPI4 fallan, se conservan las partes completadas y solo las unidades fallidas se reintentan automáticamente con una concurrencia progresivamente menor.
• Sonarpad comprueba ahora el código de salida del puente SAPI4 y rechaza las partes de audio vacías o no válidas.

Configuración del proxy
• Se añadió un campo independiente para el puerto del proxy en la configuración de red.
• El puerto puede escribirse por separado de la dirección, se valida entre 1 y 65535 y sustituye correctamente cualquier puerto ya presente en la URL.

Búsqueda de radio por idioma y país
• Los filtros Idioma y País se actualizan ahora con todas las opciones disponibles en el catálogo de Radio Browser y ya no están limitados a una lista fija.
• Los nombres de idioma se reconocen ahora aunque Radio Browser los proporcione en otro alfabeto, con su nombre nativo, como abreviaturas o como combinaciones de varios idiomas, y se muestran traducidos al idioma actual de la interfaz. Se descartan los valores que no representan idiomas reales, como números, géneros musicales, países o etiquetas genéricas.
• El catálogo se actualiza en segundo plano y conserva una lista alternativa utilizable cuando Radio Browser no está disponible.
• Las entradas de idioma de Radio Browser que quedan idénticas después de traducirse se combinan ahora en un único elemento de la lista, evitando pasos silenciosos con los lectores de pantalla.

Mejora principal: sincronización entre la lectura y el cursor
• La sincronización entre la lectura por voz y el desplazamiento del cursor se ha mejorado de forma significativa para todos los motores de voz compatibles.
• Cuando está activada la opción “Mover cursor durante la lectura”, Sonarpad utiliza ahora un sistema de avance común para Microsoft Edge Neural, Google TTS, SAPI4, SAPI5 y OneCore.
• El cursor sigue con mayor precisión el texto que se está pronunciando, con una división más coherente de frases y fragmentos.
• Se han reducido notablemente los adelantos, retrasos, saltos irregulares y diferencias entre motores de voz.
• La posición correcta se conserva mejor después de pausar, reanudar, buscar en el documento o cambiar el motor de voz.

Grabación de podcast en archivos separados
• Añadida la opción “Guardar el micrófono y el audio del sistema o de las aplicaciones en archivos separados”.
• Al grabar juntos el micrófono y otra fuente, Sonarpad puede crear un archivo solo con el micrófono y otro con el audio del sistema, de una aplicación o de las aplicaciones seleccionadas.
• La separación está disponible tanto en MP3 como en WAV.
• Si la opción está desactivada, se sigue creando un único archivo mezclado.
• Los archivos separados facilitan el ajuste de volumen, la eliminación de ruido y la edición posterior de podcasts, entrevistas y tutoriales.

Grabaciones de radio programadas
• Ahora es posible programar grabaciones de radio con antelación.
• Se pueden elegir la emisora, el día, la hora y los minutos de inicio y la duración.
• Está disponible una duración personalizada de 1 a 1.440 minutos.
• Las grabaciones pueden ejecutarse una vez, cada día o cada semana.
• La ventana muestra con mayor claridad las grabaciones activas y programadas, la fecha y hora previstas, la duración y el tiempo restante.
• Se puede utilizar el Programador de tareas de Windows para iniciar automáticamente la grabación aunque Sonarpad no esté abierto.

Calendario
• Añadido un calendario completo y accesible mediante teclado.
• Permite consultar días anteriores y siguientes, volver rápidamente a hoy y conocer fiestas y efemérides.
• Añadidos el santo y la cita del día, que se pueden leer, escuchar o copiar.
• Los recordatorios se pueden crear, modificar, eliminar, posponer y marcar como completados.
• Los avisos pueden mostrarse a la hora exacta o con antelación y funcionar mediante la programación de Windows incluso con Sonarpad cerrado.

Tiempo
• Añadida una sección de previsión meteorológica.
• Se puede buscar una ciudad y recuperar rápidamente las localidades consultadas recientemente.
• Incluye situación actual, temperatura, valores mínimo y máximo, humedad, probabilidad de precipitación y previsión para los días siguientes.
• Se puede elegir Celsius, Fahrenheit o selección automática.

Películas en cartelera
• Añadida una sección para consultar películas actualmente en los cines y próximos estrenos.
• Incluye búsqueda por título, argumento, fecha de estreno y reproducción del tráiler.

Síntesis de voz de Google
• Integrado Google TTS para leer documentos y crear audiolibros.
• Añadido un gestor para mostrar las voces, filtrarlas por idioma, descargarlas y eliminar las que ya no sean necesarias.
• Se pueden ajustar velocidad, volumen y tono.
• El tono de las voces Google Natural se aplica directamente mediante el motor para obtener un resultado más natural y estable.
• Mejoradas la rapidez y la fiabilidad de Google TTS, adaptando los límites de síntesis a la velocidad seleccionada.
• Reducidos los tiempos de espera innecesarios y mejorada la gestión de errores e interrupciones.

Índice de documentos EPUB
• Sonarpad reconoce ahora el índice incorporado en los libros EPUB.
• Su presencia se anuncia y puede abrirse desde el menú Ver.
• Los capítulos y subcapítulos se muestran de forma jerárquica.
• Al pulsar Intro se accede inmediatamente al punto seleccionado.

Noticias y fuentes RSS
• Ampliada la sección Noticias con nuevas herramientas de búsqueda y organización.
• Añadida la selección del idioma de las noticias.
• Se puede buscar dentro de las fuentes RSS y consultar noticias de la propia ciudad.
• Las fuentes compartidas por la comunidad se pueden explorar, añadir a la colección personal y enviar a la comunidad Sonarpad.

Grabación de podcast
• Se puede grabar solo el micrófono, todo el audio del sistema, una aplicación, varias aplicaciones seleccionadas o el micrófono y las aplicaciones al mismo tiempo.
• Es posible elegir el dispositivo y la fuente, ajustar por separado los volúmenes y controlar los niveles en tiempo real.
• Añadidas pausa y reanudación, salida MP3 o WAV, selección del bitrate MP3 y carpeta de destino.
• El equipo puede mantenerse activo durante la grabación.

Radio
• La sección Radio se ha reorganizado profundamente.
• Las emisoras se pueden buscar por nombre o texto libre, idioma, país, ciudad, género musical o categoría.
• Mejorada la gestión de favoritos y añadido el restablecimiento rápido de todos los filtros.
• Se pueden enviar emisoras a la comunidad Sonarpad.
• Añadidas la grabación en directo, la modalidad “Grabar y reproducir”, la lista de grabaciones y su gestión y eliminación.
• Las grabaciones de radio se guardan en una carpeta propia dentro del directorio general de grabaciones.

Reproducción multimedia
• Mejorada de forma considerable la estabilidad del reproductor multimedia.
• Corregido un problema que podía bloquear mpv y mejorada la comunicación con el reproductor.
• Mejorada la apertura de distintos tipos de archivos multimedia.
• Sonarpad recuerda ahora el volumen utilizado durante la reproducción.
• Mejorada la gestión de flujos y grabaciones.
• Corregida la apertura de archivos desde Windows mediante doble clic o “Abrir con”.

Documentos PDF
• Añadido el reconocimiento de campos de formulario en los PDF.
• Sonarpad puede localizar campos rellenables, presentarlos de forma textual accesible, permitir su modificación y guardar los datos en el PDF.
• Corregido el cálculo de la posición del cursor durante la lectura, especialmente con caracteres multibyte y estructuras complejas.

Accesibilidad y teclado
• Mejorado el funcionamiento de los comandos normales de edición en todo el programa.
• Copiar, cortar, pegar, seleccionar todo, deshacer y rehacer se envían correctamente al campo que tiene el foco, incluso en ventanas secundarias y cuadros de diálogo.
• Corregido un problema de actualización de las pantallas Braille.
• Mejorada la gestión del foco y corregida la selección de idioma en Wikipedia.
• Añadida la posibilidad de agrupar por categoría las funciones del menú Herramientas.
• Añadidas acciones configurables para abrir rápidamente Calendario, Tiempo y Películas en cartelera.

Audiolibros
• Mejorada la creación de audiolibros cuando hay cuadros de diálogo o ventanas modales abiertos.
• La gestión del progreso es más robusta e ignora actualizaciones de audio obsoletas.
• Google TTS también puede utilizarse para crear audiolibros con control de velocidad, volumen y tono.

Inteligencia artificial
• Actualizado el modelo Gemini predeterminado a `gemini-3.5-flash`.

Correcciones generales
• Corregidos varios bloqueos durante la reproducción con mpv.
• Corregida la apertura de algunos archivos de audio y vídeo.
• Mejorada la gestión de los comandos enviados al reproductor.
• Corregida la restauración del cursor durante la lectura.
• Mejorada la estabilidad de la creación de audiolibros.
• Mejorada la gestión general de multimedia, RSS, radio y EPUB.

Versión 0.7.1 – 2026-05-13

Novedades y mejoras
• Creado el sitio web oficial sonarpad.com, un nuevo punto de referencia para seguir las últimas novedades, descargar la última versión del programa, leer los comentarios de los visitantes y, en el futuro, escuchar también todos los podcasts de Sonarpad. En el menú Ayuda también se ha añadido la opción “Visitar sonarpad.com”, para abrir rápidamente el sitio web oficial.
• Corregido el problema por el que los archivos con acentos o caracteres especiales daban error al iniciar la transcripción de voz.
• A partir de ahora, en el menú Ver, elementos como Ajuste automático de línea y Mostrar vídeo durante la reproducción aparecerán siempre con el estado correcto, activados o desactivados.
• Mejorada la búsqueda en YouTube, permitiendo volver con Esc a la página o pantalla anterior.
• Añadida una comprobación preliminar para verificar si un vídeo se puede reproducir. También se ha mejorado la reproducción: Sonarpad ahora puede reproducir vídeos o listas marcadas como mix, que antes no se reproducían.
• Mejorada la gestión de los marcadores automáticos. Antes, si la opción Marcadores automáticos estaba activada y luego se desactivaba, esos marcadores permanecían; ahora el programa los ignora correctamente hasta que la opción se vuelva a activar. Además, al llegar al final de un archivo multimedia, el marcador se elimina automáticamente.
• Mejorada la gestión de etiquetas con los diálogos activos. Ahora Sonarpad gestiona correctamente ambas funciones, permitiendo insertar etiquetas aunque la opción de diálogos esté activa.
• Mejoradas las opciones de voz, separando claramente cada motor para que el ajuste sea más preciso. Los perfiles de voz conservan correctamente las opciones de cada motor individual: Edge, Sapi5 y Sapi4.
• Añadida una etiqueta para insertar pausas, directamente desde las opciones o desde el panel de voces pulsando Tab desde el editor. Las opciones son: 250 ms, 500 ms, 1 segundo, 2 segundos o duración personalizada.
• Corregido el comportamiento al reproducir un vídeo de YouTube e iniciar la transcripción. Ahora, al volver con Alt+Tab, el foco estará correctamente en el botón Cancelar de la transcripción en curso.
• Las transcripciones ahora se guardan automáticamente al finalizar el proceso.
• Mejorada la importación desde Wikipedia. Se puede elegir leer solo una sección y luego, desde el artículo, pulsar Esc para volver a la búsqueda, o importar todo el artículo. También se puede elegir el idioma de Wikipedia que se desea consultar.
• Añadida una sección de radios de todo el mundo, donde se podrá buscar una radio por país, idioma y género. También se podrán añadir radios locales a la base de datos de Sonarpad, para que otros usuarios puedan escucharlas. También es posible añadir una radio a favoritos.
• Añadida una sección de rutas para calcular recorridos eligiendo el medio: a pie, en bicicleta, en coche o en silla de ruedas. Se puede elegir si calcular la ruta más corta o la más rápida y si mostrar los municipios atravesados. Una vez importada la ruta, también se podrá guardar el mapa visual desde el menú Archivo, Guardar imagen.
• Añadida la opción Imprimir en el menú Archivo. Sonarpad imprimirá los archivos TXT usando el propio programa y usará el programa asociado para otros archivos, como DOCX, PDF y similares, para conservar al máximo el diseño original.
• Integrado en Sonarpad un servicio de traducción para cada documento, accesible desde el menú contextual del editor. El usuario podrá usar sin introducir ninguna clave API los servicios gratuitos DeepL y Google Translate; introduciendo una clave API de Gemini, podrá traducir usando Gemini.
• En el menú de traducción, el usuario podrá elegir el idioma de destino. El menú se reordena automáticamente: si un usuario elige primero inglés, luego francés y luego italiano, estas tres opciones aparecerán arriba en el menú de idiomas.
• Si el usuario introduce su clave API de Gemini, también podrá acceder a la función Resumir texto, disponible siempre en el menú contextual, para resumir cualquier artículo.
• Añadido en el menú Reproducir, visible al reproducir un archivo multimedia, un menú para dividir el medio actual. Funciona con MP3, MP4 y otros formatos, dividiendo por número de partes o según la duración de cada parte.

Versión 0.7.0 – 2026-04-25

Novedades
• Se añadió compatibilidad con el reproductor mpv para la reproducción en streaming. Los vídeos de YouTube y de sitios compatibles ahora se reproducen al instante; si el usuario decide conservarlos, se descargan como antes. Si se inicia la transcripción de contenido en streaming, primero se descarga y luego se transcribe. El reproductor mpv también se utiliza para abrir vídeos locales y gestionar subtítulos, garantizando una mayor compatibilidad con numerosos formatos que antes no se gestionaban correctamente.
• Mejorada la grabación de podcasts del audio del sistema: ahora es posible elegir si se quiere grabar todo el audio del sistema, una sola aplicación o varias aplicaciones al mismo tiempo. Esta opción está integrada con la grabación normal, por lo que sigue siendo posible activar o desactivar el micrófono por separado.
• Se añadió el idioma hindi. Interfaz traducida y añadidos RSS, registro de cambios y guía de Sonarpad.
• Se añadió una opción en la pestaña Editor para mover siempre el cursor al inicio de la línea al usar las flechas arriba y abajo.
• Se añadió una opción en el menú "Convertir audio" para convertir audio a M4B.

Correcciones
• En los comentarios de YouTube abiertos desde "Reproducir audio en streaming...", Sonarpad ahora carga inicialmente solo los primeros 50 comentarios principales, incluyendo siempre todas las respuestas de esos comentarios, y añade al final una opción para cargar todos los comentarios bajo demanda.
• Los marcadores ahora se muestran y se gestionan según su posición tanto en documentos de texto como en archivos multimedia, en lugar de seguir el orden de creación. Si ya existe un marcador en la misma posición, ya no se vuelve a añadir.
• Se ha añadido una opción en el menú Marcadores que, si se activa, permite gestionar los marcadores automáticamente. Al reproducir un archivo local o en streaming y cerrarlo, Sonarpad establece automáticamente un marcador según la posición alcanzada y, al volver a abrir el archivo, reanuda desde ese punto. Lo mismo ocurre con los archivos de texto: si se abre un texto y se mueve el cursor, Sonarpad recordará esa posición al cerrarlo; si se inicia la lectura, se guardará la última frase leída y la lectura continuará exactamente desde ahí.
• Se ha añadido al menú Ver una opción para mostrar el renderizado de vídeo para archivos locales o en streaming. El contenido de vídeo se muestra en una ventana ampliada, donde todos los controles permanecen ocultos salvo que se pulse la tecla Alt o se mueva el ratón hacia la parte superior de la ventana. De este modo, los usuarios con baja visión deberían disponer de un contenido más grande y más fácil de utilizar.

Versión 0.6.9 – 2026-04-08

Correcciones
• Se ha mejorado la experiencia de Buscar en archivos: al abrir Examinar carpeta, el foco va directamente a la lista de carpetas; al abrir un resultado con Intro, todos los comandos de teclado siguen funcionando; al pulsar Esc se vuelve al resultado seleccionado anteriormente; y al regresar con Alt+Tab, el foco vuelve al campo de búsqueda o a la lista de resultados si estaba abierta.
• F5 siempre iniciaba la lectura desde el principio. Ahora se ha corregido y la lectura comienza desde la posición actual del cursor, manteniendo `Ctrl+F5` y `Shift+F5` para ir a la frase anterior o siguiente.
• Después de usar Ir a la línea, al pulsar Esc el foco podía salir de Sonarpad. Ahora vuelve correctamente al editor.
• La opción `Ajuste de línea` ahora se aplica inmediatamente también a los documentos ya abiertos, sin necesidad de reabrir el archivo.

Versión 0.6.8 – 2026-04-07

Novedades
• Añadido un nuevo elemento en el menú Reproducir que permite transcribir cualquier archivo de audio o vídeo con Whisper. En Opciones hay una nueva sección llamada «IA y transcripción», donde se puede elegir el modelo, habilitar la compatibilidad opcional con CUDA para tarjetas gráficas NVIDIA, mantener el idioma original y activar o desactivar las marcas de tiempo.
• Añadida en el menú Reproducir la nueva acción `Transcribir carpeta actual`, que transcribe todos los archivos de audio compatibles de la carpeta del medio abierto y los une en un único documento, con ventana de progreso dedicada, indicación del archivo actual y posibilidad de cancelar. También puede iniciarse con `Alt+Shift+C`.
• Añadida la posibilidad de usar el dictado por voz sin conexión, con el mismo funcionamiento que la transcripción de audio. De forma predeterminada, se pulsa `Ctrl+Shift+Espacio` para iniciar el dictado y se vuelve a pulsar el mismo atajo para detenerlo; el atajo puede personalizarse en Opciones. A partir de la segunda activación, el dictado es más rápido porque el motor queda listo en memoria; en los PC con menos de 4 GB de RAM esta precarga y reutilización se desactivan automáticamente.
• Añadida en las Opciones del editor una nueva configuración, desactivada por defecto, que hace que `Esc` cierre la ventana del editor.
• La búsqueda de podcasts ahora usa `iTunes + Spreaker` por defecto, con filtrado de resultados duplicados cuando el mismo podcast aparece en ambas plataformas.
• Mejorada la búsqueda y exploración de podcasts Apple: la búsqueda de podcasts, la navegación por categorías y los top podcasts por categoría ahora usan el país seleccionado para el directorio de podcasts. En Opciones > RSS / Podcast se puede dejar en `Automático` para usar el país del sistema o elegir manualmente otro país.
• Se aumentó el límite de resultados para las categorías de podcasts de Apple. La primera apertura sigue cargando los primeros 50 resultados como antes; si eliges `Cargar más resultados`, Sonarpad carga hasta 200 resultados en total (límite impuesto por Apple) y permite navegar por las páginas siguientes con una experiencia más fluida.
• Sonarpad ya está disponible también en Mac, aunque con un conjunto de funciones parcial. Enlace del proyecto: https://github.com/Ambro86/Sonarpad-Mac

Mejoras
• Se añadieron más de 50 países seleccionables para el directorio de podcasts, de modo que ahora se puede elegir entre muchos más catálogos nacionales.
• "Reproducir audio en streaming..." ahora también permite buscar en YouTube escribiendo cualquier texto o pegar el enlace de un canal o una lista de reproducción de YouTube para mostrar sus resultados.
• Se mejoró la visualización de los resultados en "Reproducir audio en streaming...": las entradas de YouTube ahora incluyen título, duración, canal y visualizaciones en un formato más claro.
• "Reproducir audio en streaming..." ahora también admite los comentarios de YouTube: se pueden abrir desde el menú contextual, leer las respuestas y expandir los hilos de comentarios con la Flecha derecha.
• Se añadieron favoritos de YouTube para canales y listas de reproducción en "Reproducir audio en streaming...": pueden añadirse desde los resultados mediante el menú contextual, abrirse directamente desde la lista Favoritos accesible con Tab justo después del campo URL/consulta de YouTube y eliminarse más tarde desde esa misma lista también con el menú contextual. En los resultados de búsqueda de YouTube, el menú contextual solo está disponible para canales y listas de reproducción.
• "Reproducir audio en streaming..." ahora puede pedir credenciales cuando un sitio requiere iniciar sesión. El usuario puede introducirlas, guardarlas para ese sitio y gestionar después las credenciales guardadas en Opciones > Audio.
• Mejorado el enfoque durante "Reproducir audio en streaming...", para que la ventana de progreso se mantenga más estable durante la descarga y la conversión.
• Añadidas dos nuevas acciones de lectura en el menú Voz: `Frase anterior` y `Siguiente frase`, con atajos configurables para saltar durante la lectura del texto.
• El atajo predeterminado de `Ejecutar archivo con intérprete` ahora es `Ctrl+Shift+F5`, para que `Shift+F5` pueda usarse por defecto para `Frase anterior`.
• Añadida la gestión de perfiles de voz en Opciones > Voz: se pueden añadir, renombrar y eliminar perfiles.
• Ampliadas en Opciones > Audio las opciones del intervalo de rebobinado durante la reproducción, con nuevos valores desde 1 segundo hasta 2 horas.
• Añadida la traducción rusa gracias a Dmitriy.
• Añadida en Opciones > Audio una nueva opción para elegir el formato del nombre de las partes del audiolibro: `Título + número`, `Solo número` o `Número + título`.
• Añadida la acción del menú contextual de artículos RSS para añadir un artículo a favoritos.
• La fuente RSS "Favoritos" puede eliminarse y se recrea automáticamente al añadir un nuevo artículo a favoritos.
• Añadidos atajos de teclado RSS para mover las fuentes arriba/abajo: `Ctrl+Shift+Flecha arriba` y `Ctrl+Shift+Flecha abajo`.
• Mejorada la ventana RSS con una vista previa integrada del artículo, para consultar el texto directamente allí y alcanzarlo rápidamente con Tab antes de abrir el artículo completo en el editor.
• Añadida en RSS una entrada explícita «Cargar más noticias» al final de las fuentes cuando hay más elementos disponibles; al pulsar Intro se carga el siguiente bloque y el foco se mueve al primer artículo nuevo.
• En el diccionario de voz, al añadir o editar una sustitución, ahora hay una casilla «Distinguir mayúsculas y minúsculas» para decidir si cada sustitución debe respetar o ignorar el uso de mayúsculas.
Correcciones
• "Reproducir audio en streaming..." ahora respeta el límite de caché de podcasts ya configurado en Opciones, y ese mismo límite también se aplica a la reproducción de audiodescripciones.
• Corregida la importación desde Wikipedia, que en algunas páginas no importaba correctamente las citas presentes en el texto.
• Mejorado el parser de páginas web: en algunas páginas WordPress no se incluían los elementos de listas ni algunos títulos de sección.
• Ahora, al usar «Ir a la línea», el campo se rellena automáticamente con la línea actual.
• Corregida la exportación OPML de podcasts y RSS, que ahora genera archivos aceptados por iTunes.
• Corregida la transcripción de archivos multimedia: ahora, al cerrar con Alt+F4 el documento generado, Sonarpad pregunta si se quiere guardar el archivo y propone el nombre correcto basándose en el nombre del archivo transcrito, en lugar de la primera línea del texto.
• Añadidos mensajes de confirmación localizados para la correcta importación y exportación OPML de fuentes RSS y podcasts.
• Corregido un problema por el que, en "Reproducir audio en streaming...", al escribir un texto de búsqueda y seleccionar un canal de YouTube en los resultados, el programa podía parecer bloqueado en lugar de abrir los vídeos del canal.
• Corregido un error por el que la lista de archivos abiertos se mostraba en el menú Ayuda en lugar del menú Ventana.
• Corregido un caso límite de streaming en el que la reproducción podía iniciarse pero la ventana “Descargando streaming” quedaba abierta cuando el archivo descargado ya coincidía con el formato de destino.
• Corregido el comportamiento de conversión en streaming MP3: cuando el stream ya es MP3 y el usuario elige un bitrate MP3 explícito (por ejemplo 128 kbps), Sonarpad ahora recodifica al bitrate seleccionado en lugar de omitir la conversión.
• Corregido el atajo `Alt+Shift+L`: ahora abre correctamente la lista de capítulos durante la reproducción.
• Corregido el atajo `Alt+Shift+T`: ahora inicia correctamente «Transcribir audio actual» en lugar de abrir el menú Herramientas.
• Si ya se está reproduciendo un audio, al iniciar la transcripción Sonarpad ahora pone ese audio en pausa automáticamente antes de comenzar.
• Corregido un problema por el que, al importar un artículo desde Wikipedia, la importación podía completarse pero el texto del artículo no se mostraba en pantalla.
• Añadido soporte para capítulos de podcast incrustados en archivos multimedia locales (por ejemplo, metadatos de capítulos MP3): cuando el feed/URL no ofrece capítulos, Sonarpad ahora los carga desde el archivo descargado en segundo plano, de modo que la reproducción comienza de inmediato y los capítulos se aplican en cuanto están listos.
• Corregida la carga de capítulos para episodios de podcast descargados y abiertos como archivos multimedia locales normales: los capítulos incrustados ahora también están disponibles en ese caso, no solo al iniciar la reproducción desde la ventana Podcasts.
• Corregida la finalización de audiolibros MP3 con SAPI4 y SAPI5: el archivo final ahora se finaliza correctamente, evitando archivos incompletos o frágiles después de exportaciones largas.
• Añadida una barra de progreso explícita para la fase de finalización en todos los modos de creación de audiolibros: después de la creación, Sonarpad anuncia y muestra la finalización con progreso visible.
• Corregido un error en las voces de diálogo: los parámetros de velocidad/tono/volumen de la primera y segunda voz de diálogo ahora se aplican correctamente durante la síntesis.
• Mejorada la detección de codificación para archivos japoneses `.txt`: se añadió un fallback seguro Shift_JIS/CP932 en casos de mojibake, preservando el comportamiento existente para UTF/diacríticos/chino.
• Refactorización interna de seguridad: conversión a implementaciones seguras donde fue posible y reducción drástica de líneas de código unsafe.

Versión 0.6.7 – 2026-03-02
Mejoras
• Ora il programma riesce a gestire Sostituisci tutto in modo massivo su file grandi con un gran numero di sostituzioni.
• Traducción polaca actualizada gracias a DJ Graco.
• Añadida la traducción lituana.
• Añadida la traducción china.
• A partir de ahora, se publicarán compilaciones beta frecuentes en la sección Releases del proyecto, para que los usuarios puedan probar los nuevos cambios antes de la próxima versión estable.
• Añadido el atajo `Ctrl+.` para insertar el carácter de puntos suspensivos (…).
• Mejorado el soporte de capítulos en podcasts: la navegación por capítulos ahora es más fiable, también en episodios directos/streaming donde los capítulos no están incrustados en el archivo MP3, usando metadatos de capítulos desde feed/URL como fallback cuando están disponibles. Añadidos los atajos `Ctrl+Alt+PageUp` (capítulo anterior) y `Ctrl+Alt+PageDown` (capítulo siguiente).
• Reorganizadas las carpetas de salida en `Documentos\\Sonarpad`: los archivos ahora se guardan en subcarpetas dedicadas `audiobooks`, `documents`, `recordings` y `media`, con migración automática desde rutas heredadas.
• Mejorado el soporte para archivos de texto muy grandes (incluidos 60 MB): apertura y navegación línea por línea más fluidas, especialmente con lectores de pantalla.
• Guías actualizadas para todos los idiomas y recursos de localización actualizados en toda la app, incluidas donaciones y traducciones del instalador NSIS (nuevas cadenas de instalación en chino simplificado y lituano, además de completar la traducción ucraniana del setup).
• Añadido soporte global de proxy de red (HTTP/HTTPS y SOCKS5/SOCKS5H) para funciones en línea, con validación al guardar Opciones: los proxies no válidos se avisan y se eliminan automáticamente.
• Añadida una nueva función en Herramientas: "Reproducir audio en streaming...", que permite pegar una URL (YouTube o enlace multimedia directo), elegir el formato de salida y el perfil de calidad/bitrate (incluida calidad/bitrate original para MP3 y MP4) y reproducirla en el reproductor de Sonarpad.
• Añadido soporte para la tecla multimedia de sistema Reproducir/Pausar (auriculares/teclado): ahora controla tanto la reproducción multimedia como la pausa/reanudación de la lectura de texto (con prioridad para el reproductor multimedia cuando ambos están activos).
• Añadida en Archivo > Archivos recientes la nueva opción "Limpiar archivos recientes" para vaciar rápidamente la lista de documentos recientes.
• Ampliadas las opciones de bitrate en la conversión de audio y en la grabación de podcast: añadidos valores más bajos (64/96 kbps) y ampliado MP3 hasta 320 kbps, con validación y manejo del codificador alineados.
• Ampliadas las opciones de división de audiolibros por tiempo hasta 60 minutos.
• Mejorada la división de audiolibros por partes: ahora se puede introducir manualmente el número de partes, con validación de 1 a 100.
• Añadido el nuevo modo Ver > Solo lectura para bloquear ediciones accidentales del texto manteniendo la lectura y navegación completas de los documentos.
• Añadida una barra de progreso accesible durante las actualizaciones del programa, para que los lectores de pantalla puedan seguir en tiempo real el avance de la descarga.
• Añadida una nueva barra de estado discreta en la ventana principal con recuento de caracteres, palabras y posición línea/columna (por ejemplo: "Caracteres (con espacios): 11. | Palabras: 2. | Ln 1, Col 12"), sin interferir con el foco de NVDA.
• Añadida una nueva opción en el menú Ver para Ajuste de línea, que permite activar o desactivar rápidamente el ajuste sin abrir Opciones.
• Añadidas en Editar > Texto nuevas acciones para aumentar/reducir sangría, con atajos Ctrl+Shift+. (indentar) y Ctrl+Shift+, (desindentar), porque cuando “Mostrar voces en el editor” está activo la tecla Tab queda reservada para navegar el panel de voces.
• Añadida la visualización localizada de fecha y hora en artículos RSS y episodios de podcast, con formato adaptado al idioma de la interfaz.
• Añadida en el menú contextual RSS una nueva acción para compartir por correo electrónico el artículo seleccionado.
• Añadidas opciones granulares de confirmación de borrado en Opciones > RSS y podcast: para RSS (feed/artículo/ambos/ninguno) y para Podcasts (podcast/episodio/ambos/ninguno).
• Añadida copia rápida RSS configurable con Ctrl+C (Opciones > RSS y podcast): copiar titular, URL, contenido del artículo o todo junto.
• Unificado el flujo de RSS: “Añadir fuente” ahora acepta tanto URL de feed como palabras clave (generando automáticamente el feed de Google News), sin necesidad de una búsqueda separada.
• Al pulsar Ctrl+A ahora se anuncia la finalización de la acción para ofrecer una respuesta más clara en lectores de pantalla.
• Añadido Shift+F3 para "Buscar anterior" en el menú Editar, junto a F3 "Buscar siguiente".
• Mejorado el mensaje de reemplazo con singular/plural correcto (por ejemplo, “1 reemplazo realizado” frente a “2 reemplazos realizados”).
• Añadida en la ventana del diccionario la selección de idioma de búsqueda, con valor predeterminado Auto (idioma de la interfaz) y opción de ajuste manual.
• Añadida una nueva pestaña de Atajos en Opciones para personalizar combinaciones de teclas, con detección de conflictos y aviso cuando un atajo ya está asignado a otra acción.
• Añadido soporte inicial para parámetros de línea de comandos: `-h`/`--help` muestran la ayuda rápida y `--version` muestra la versión del programa.
• Mejorada la claridad del ajuste manual de velocidad y tono: los campos manuales ahora usan una escala centrada en 100, donde 100 corresponde al valor normal.
• Mejorada la selección de voces Microsoft tanto en Opciones > Voz como en el panel de voces del editor: se añadió un combobox de idioma localizado para filtrar voces por idioma, manteniendo el modo “solo voces multilingües” como una lista única sin división por idioma (con el combobox de idioma oculto cuando está activo).
• Añadida la configuración de voz para diálogos en Opciones > Voz con navegación completa por Tab, usando el mismo modelo de voces de la interfaz principal (motor, filtro de idioma Edge, voz y velocidad/tono/volumen con etiquetas); añadida también una segunda voz de diálogos opcional con los mismos controles (motor, filtro de idioma Edge, voz, velocidad/tono/volumen) para alternar diálogos; las reglas de diálogos se guardan en configuración `.ini`, sin modificar el texto del documento.
• Mejorada la etiqueta de Deshacer: la opción Editar > Deshacer ahora muestra qué acción se va a deshacer (por ejemplo, edición de texto, comentar/descomentar líneas o inserción de etiqueta de voz), manteniéndose no disponible cuando no hay nada que deshacer.
Correcciones de errores
• Corregido el soporte de apertura RTF: los archivos `.rtf` ahora se extraen y se muestran como texto legible, no como marcado RTF en bruto (p. ej. `{\\rtf1...}`).
• Corregida la apertura de archivos de texto chinos codificados en GB18030/GBK: Sonarpad ahora los detecta y decodifica correctamente, evitando texto ilegible (mojibake).
• Mejorada la creación de audiolibros M4B con metadatos y marcadores de capítulo; corregido el problema "chipmunk" (voz demasiado aguda/rápida) en los archivos M4B generados.
• Corregida la interfaz de bitrate en la ventana de guardado de audiolibros: se eliminaron textos hardcoded en italiano y se añadió 64 kbps entre los bitrates seleccionables.
• Corregido "Guardar todo" (Ctrl+Shift+S): ahora se detectan de forma fiable todos los documentos abiertos modificados (incluidas pestañas nuevas/sin guardar) y Guardar todo guarda cada uno correctamente, abriendo "Guardar como" cuando hace falta.
• Corregido el orden de artículos RSS de Google News: cuando hay fecha disponible, los artículos ahora se muestran del más reciente al más antiguo.
• Corregida la asociación de etiquetas en NVDA en la ventana del diccionario: el campo de búsqueda y el combo de idioma ahora anuncian la etiqueta correcta.
• Corregida la navegación de teclado en la ventana Propiedades de RSS/Podcast: Tab/Shift+Tab ahora llegan al botón Aceptar, Enter activa Aceptar, Esc cierra de forma segura y el foco vuelve correctamente a la lista RSS/Podcast.
• Corregido el historial de deshacer en RSS/Podcast: Ctrl+Z ahora admite deshacer multinivel para eliminaciones (artículos/episodios y fuentes), no solo la última acción.
• Mejorados los avisos de eliminación en RSS/Podcast con mensajes explícitos (RSS eliminado, artículo RSS eliminado, episodio de podcast eliminado).
• Mejorado el foco tras eliminar/deshacer en RSS/Podcast: en RSS se selecciona de forma fiable el primer feed cuando hace falta y se reducen repeticiones de anuncios del lector de pantalla durante la reselección diferida.

Versión 0.6.6 – 2026-02-13
Mejoras
• Añadida "Formateo automático para TTS" en el menú Editar para preparar rápidamente el texto para voz (elimina markdown/comillas y recompone líneas partidas).
• Mejorada la inserción de etiquetas de voz: cuando hay texto seleccionado, ahora las etiquetas se aplican correctamente tanto a una sola línea como a selecciones multilínea.
• Añadida una opción en Configuración de audio para elegir la carpeta predeterminada de guardado de audiolibros (predeterminada: Documentos\\Sonarpad Audiobooks).
• En el cuadro de guardado de audiolibro, cuando está activa la división en partes, se añadió una nueva opción (activada por defecto) para crear una subcarpeta dedicada a las partes generadas.
• La exportación de audiolibros ahora guarda MP3 en estéreo con bitrate elegido por el usuario para voces Edge, SAPI5 y SAPI4.
• Añadido soporte para voces SAPI5 de 32 bits mediante bridge, para usar también voces disponibles solo en motores de 32 bits.
• Reorganizadas las funciones de voz en un menú dedicado "Voz y audio" y añadida/aclarada la opción "Convertir audio", útil para convertir cualquier archivo multimedia compatible a MP3, AAC, OGG, Opus, FLAC, WAV y AIFF.
• Añadida la eliminación de artículos RSS individuales y episodios de podcast individuales (tecla Supr + menú contextual con confirmación), sin eliminar toda la fuente RSS/podcast, con deshacer de la última eliminación (artículo/episodio individual o fuente RSS/podcast completa).
• Añadida la exportación de feeds RSS a OPML en la ventana RSS, para guardar y reimportar fácilmente las fuentes actuales.
• Añadida la función "Buscar RSS por palabra clave" en la ventana RSS: al introducir una palabra clave se genera automáticamente la URL RSS de Google News y se abre el diálogo de añadir fuente ya prellenado, para crear feeds temáticos en un solo paso.
• Añadida la traducción serbia gracias a Mila Kuran.
• Añadida la traducción ucraniana gracias a Ivan Shtefuriak.
• Añadida la apertura múltiple de archivos multimedia: al abrir varios archivos juntos se crea una cola de reproducción en lugar de sustituir el archivo actual.
• Añadidos atajos de salto variable durante la reproducción: con base de 1 minuto, Izquierda/Derecha salta 60s, Shift+Izquierda/Derecha salta 20s y Ctrl+Izquierda/Derecha salta 3 minutos.
• Añadidos atajos para pista anterior/siguiente en el reproductor: Ctrl+PageUp y Ctrl+PageDown.
• Añadida la opción "Restablecer volumen" y agrupadas las acciones de reinicio en un submenú dedicado "Restablecer" en Reproducción, junto con "Restablecer velocidad" y "Restablecer tono".
• Mejoras del instalador: setup.exe ahora permite elegir entre asociar todos los tipos de archivo compatibles o seleccionar manualmente las extensiones; el MSI también ofrece selección por extensión en el árbol de características (el valor predeterminado se mantiene: todas activadas).
• Añadido el nuevo menú "Ventana" con la opción "Documentos abiertos..." para cambiar rápidamente a cualquiera de los archivos abiertos.
• Actualizada la opción Ver > Fuente: se reemplazó el selector completo por un submenú rápido con fuentes comunes (Arial, Calibri, Consolas, Segoe UI, Tahoma, Verdana, Times New Roman, Georgia), manteniendo el tamaño de texto actual.
• Mejorada la lectura de RSS y podcasts con dos avisos diferenciados: los nodos de fuente anuncian "nuevos elementos" cuando un feed/podcast tiene novedades, mientras que los artículos RSS y episodios de podcast individuales anuncian "no leído"/"no reproducido"; este comportamiento se puede desactivar desde Opciones.
Correcciones de errores
• Corregida la extracción de texto EPUB para libros con comentarios HTML inline (<!-- ... -->): ahora el texto de los capítulos se analiza correctamente en lugar de omitirse parcial o totalmente.
• Corregido el diccionario Wiktionary en español y la caché del diccionario: palabras como "agua" ahora se encuentran correctamente y las entradas antiguas de "Palabra no encontrada" ya no se reutilizan.
• Corregida la codificación al importar artículos RSS en algunas fuentes españolas (p. ej., El Mundo): los acentos y la "ñ" ahora se muestran correctamente en el editor temporal.
• Corregida la decodificación ANSI de archivos centroeuropeos (p. ej., checo/polaco): Sonarpad ahora distingue mejor entre UTF-8 y ANSI y elige la página de códigos correcta (incluida Windows-1250), evitando diacríticos corruptos.
• Corregida la persistencia de fuentes RSS con parámetros en la URL (p. ej., `rss.aspx?c=...`): estos feeds ahora se guardan y restauran correctamente tras reiniciar Sonarpad.
• Corregida la apertura de archivos puntero de Google Drive (`.gdoc`, `.gsheet`, `.gslides`) desde el menú contextual del Explorador: si la lectura directa falla con “Incorrect function (os error 1)”, Sonarpad ahora usa un fallback por shell-open y el documento se abre correctamente.
• Corregida la lectura de archivos Excel legacy `.xls` (Excel 2010): los archivos binarios antiguos ahora se detectan y decodifican correctamente en lugar de mostrar texto corrupto (p. ej. `ÐÏ_à¡±...`).
• Corregido el flujo de anuncios del corrector ortográfico: los errores vuelven a anunciarse al revisar el texto más tarde, y el mismo error se informa de nuevo si se borra y se vuelve a escribir.
• Corregidas las operaciones de texto por líneas (p. ej. Ctrl+Q / Ctrl+Shift+Q, ordenar/invertir/únicas/unir líneas): al seleccionar una sola línea con Mayús+Flecha abajo ya no se unen ni se truncan las líneas adyacentes.
• Corregido el comportamiento multinea en operaciones de texto por líneas (Ctrl+Q / Ctrl+Shift+Q y herramientas relacionadas): cuando RichEdit entrega separadores de línea solo CR, ahora se normalizan correctamente y se procesan todas las líneas seleccionadas sin cortar el primer carácter.
• Ampliada la normalización de entrada TTS para símbolos visibles de espacio/tab/nueva línea (␠/U+2420, ␣/U+2423, ␉/U+2409, ␊/U+240A, ␍/U+240D, ␤/U+2424), que con voces multilingües podían causar repetición de párrafos.
• Refinada la sanitización del texto para Edge TTS con una única tubería de validación: normalización de espacios extraños/invisibles, compactación de secuencias largas de puntuación (como "...", "!!!", "???") y omisión de fragmentos formados solo por puntuación para evitar bucles de reproducción.
• Corregido el anuncio del tiempo de reproducción (Ctrl+I) para streams MP3/podcast: el tiempo actual ahora se limita a la duración de la pista y la reproducción se detiene automáticamente si la posición supera el final.
• Mejorada la cobertura de localización del instalador: setup.exe ahora incluye también checo, polaco, francés y serbio, mientras que el MSI se mantiene como un único paquete en-US para evitar confusión en las releases.
• Corregida la limpieza en desinstalación de entradas del menú contextual: "Abrir con Sonarpad" ahora se elimina de forma fiable, también en escenarios de registro heredados.
• Corregida la fiabilidad de pausa/reanudar en SAPI5: la pausa con F4 ahora funciona correctamente y al reanudar vuelve al punto esperado en lugar de reiniciar desde el principio.
• Corregido el flujo pausa + salto + reanudar en la reproducción multimedia: tras pausar y mover con Izquierda/Derecha, al pulsar Espacio ahora reanuda de forma fiable desde la posición actual en lugar de detenerse o reiniciar desde el inicio.

3. Añadido el servicio opcional Sonarpad AI para las audiodescripciones con IA. Los usuarios pueden seguir usando su propia clave API de Gemini o elegir el servicio Sonarpad e introducir un código Sonarpad personal. El código se protege con Windows DPAPI. En el modo de servicio, los fragmentos de vídeo preparados se cargan directamente desde el PC del usuario a Google mediante una URL temporal de carga de Google; sonarpad.com no recibe ni almacena los bytes del vídeo. El backend solo autoriza el acceso, impone el modelo Gemini/Standard y los límites de uso, contabiliza el consumo y devuelve la respuesta de Gemini.

Version 0.6.5 – 2026-02-07
Mejoras
• Traducción al español mejorada gracias a Arturo Fernandez Rivas.
• Se agregó una opción para dividir audiolibros EPUB por capítulos.
• Las importaciones RSS ahora usan una pestaña temporal dedicada (título localizado); Guardar como la convierte en un documento normal.
• Los mensajes del lector de pantalla ahora también se envían a JAWS cuando está disponible.
Correcciones de errores
• La lectura desde el cursor (F5) ahora empieza exactamente en el cursor. Antes podía comenzar un par de líneas arriba porque el desplazamiento del cursor no coincidía con las posiciones CRLF/UTF-16.
• Corregido un problema de redibujado: al escribir sobre una selección, el texto anterior podía desaparecer hasta mover la selección.
• Corregido el parseo de capítulos EPUB: las páginas de portada o solo con imágenes ya no generan lectura de CSS (p. ej., "padding") ni títulos "Sconosciuto".
• Corregido el fallo al dividir por tiempo audiolibros desde EPUB: Edge TTS podía fallar con chunks vacíos o demasiado largos ("Edge audio not sent").
• Se decodifican las entidades HTML en los artículos RSS (por ejemplo &quot;, &amp;, &lt;, &gt;).
• Guardar/Guardar como ahora propone el nombre del archivo existente al guardar formatos no sobrescribibles (p. ej., EPUB), en lugar de la primera línea.
• Se corrigió un problema por el que los podcasts con nuevos episodios no se anunciaban como no reproducidos, y se renombró "No escuchado" a "No reproducido" por ser más profesional.

Version 0.6.4 – 2026-02-05
Mejoras
• El programa se ha renombrado a Sonarpad para dar mayor enfasis al sonido y al audio, que son la clave de este programa.
• Añadida la selección de pistas de audio en el menú Reproducción para archivos multimedia con múltiples pistas de audio (ej. MKV con varios idiomas).
• Los podcasts ahora indican claramente los no escuchados con el prefijo "No escuchado" antes del nombre.
• Nuevo sistema de etiquetas para cambiar la voz en el texto. Ejemplos:
  - Voces Microsoft (Edge): <voice edge it-IT-IsabellaNeural>Hola</voice>
  - Voces SAPI5: <voice sapi5 Microsoft Helena Desktop>Hola</voice>
  - Voces SAPI4: <voice sapi4 #1>Hola</voice>
  - Con velocidad/tono/volumen: <voice edge it-IT-ElsaNeural speed=-20 pitch=-5 volume=-10>Hola</voice>
• Enriquecidas las categorias de podcasts.
• Añadida una opción en el menú contextual para crear un audiolibro desde la selección.
• Añadida la división de audiolibros por duración, con la posibilidad de elegir el nombre del primer archivo.
• Localizada la etiqueta del autor en la lectura de artículos (ej. "por", "by", "di").
• Añadidas opciones de indentación (tabulaciones/espacios con anchura) y Tab/Mayús+Tab para indentar/desindentar líneas seleccionadas.
• Corregida la limpieza de Markdown: ahora gestiona los bullets '*' cuando no se conservan las listas.
Correcciones de errores
• Corregido un error por el cual los audiolibros con SAPI4 podian crearse de forma diferente a lo esperado.
• Ventana Buscar en archivos: al pulsar Intro en un resultado ahora abre en la posición correcta del fragmento y Esc vuelve a los resultados.
• Ventana Opciones: se ajustó el diseño visual de las pestañas General, Voz, Editor y Audio para evitar controles faltantes o recortados.
• Corregido un problema de marcadores al cambiar la velocidad de reproducción.
• Corregido un problema con Podcast Index y las categorías que no se mostraban correctamente.
• Corregido el problema del apóstrofo que cortaba la lectura: ya no hay lectura separada para diálogos, se usan las etiquetas de voz.

Versión 0.6.3 – 2026-01-30
Mejoras
• Mejorada la detección del micrófono.
• Añadida reproducción instantánea para todos los formatos.
Correcciones
• Corregido el fallo en la ventana de categorías de podcasts.

Versión 0.6.2 – 2026-01-30
Nuevas funcionalidades
• Añadida la ejecución de archivos (Shift+F5). Los usuarios pueden seleccionar un intérprete (ej. python) en Opciones, buscarlo en el ordenador, y presionando Shift+F5 se ejecuta el script actual. Los archivos HTML se abren en el navegador.
• Añadido soporte para archivos de puntero de Google Docs (.gdoc, .gsheet, .gslides), que se abren automáticamente en el navegador predeterminado.
• Añadido soporte para el formato de audiolibro M4B (Apple/AAC).
• Añadida la opción "Mostrar episodios" en el menú contextual de resultados de búsqueda de podcasts para explorar y reproducir episodios sin suscribirse.
• Añadida la función "Ir a línea" (menú Editar o Ctrl+J) para saltar rápidamente a un número de línea específico.
• Añadidas opciones en el menú contextual para ordenar feeds RSS y podcasts (alfabéticamente o por fecha).
• Añadidos feeds RSS predeterminados en vietnamita.
• Añadida una casilla de prueba de micrófono en el diálogo de grabación para verificar los niveles antes de comenzar.
• Añadida "Mostrar descripción" para episodios de podcast en el menú contextual.
• Añadido soporte para formatos de audio/video extendidos vía FFmpeg: mkv, avi, mov, m4v, webm, mpg, ts, wmv, flv, vob, 3gp, flac, ogg, wma, aiff.
• Añadida lectura sincronizada de subtítulos (srt, vtt, ass, sub, sbv, lrc, smi) con NVDA o voz seleccionada. El programa busca un archivo de subtítulos con el mismo nombre que el archivo multimedia. Añadidas las opciones "Importar subtítulos" y "Eliminar subtítulos" en el menú Reproducción para archivos con nombres diferentes.
• Añadidas asociaciones de archivos para todos los nuevos formatos de audio/video soportados en el menú contextual "Abrir con Sonarpad".
• Añadida configuración para ajustar el pitch de cualquier archivo.
• Añadida opción en Configuración General para activar o desactivar los informes de errores anónimos. Añadida una entrada en el menú Ayuda para crear un archivo ZIP de diagnóstico.
• Añadida opción para usar una voz diferente para los diálogos, tanto para la lectura en vivo como para la creación de audiolibros.
• Añadido el explorador de categorías de podcasts para explorar podcasts por categoría (negocios, arte, deportes, etc.).
Mejoras
• Abrir un archivo de audio/video desde el Explorador ahora abre directamente la vista del reproductor en lugar del editor de texto.
• Eliminada la solicitud de OCR para PDFs no accesibles; el OCR ahora se realiza automáticamente para mejorar la velocidad y experiencia del usuario.
• Mejorada la Terminal Accesible: la lectura NVDA ahora recuerda la última línea leída para mejor continuidad.
• SAPI 4: La creación de audiolibros ahora está completamente paralelizada y es casi instantánea. Añadida una solicitud para elegir el número de procesos concurrentes.
• SAPI 4: Eliminado el cuello de botella WAV-MP3 convirtiendo fragmentos en paralelo durante la síntesis.
• SAPI 4: Mejorado el manejo de errores y limpieza automática de archivos temporales.
• Diálogo Buscar: Renombrado "Regex" a "Expresión regular" para mayor claridad y añadidas las traducciones faltantes para las opciones de búsqueda.
• Audiolibros M4B: Mejor manejo de salida; dividir por partes/marcadores ahora produce un único archivo M4B con metadatos de capítulos incluyendo título y autor.
• Reproductor: Corregida la precisión de marcadores y anuncios de tiempo cuando la velocidad de reproducción no es 1.0x.
• Restaurada la navegación Ctrl+Tab y Ctrl+Shift+Tab en Opciones.
• Añadida una opción en el menú Reproducción para restablecer instantáneamente la velocidad Normal (1.0x).
• Actualizadas todas las dependencias a las últimas versiones para mejor rendimiento y estabilidad.
• Integrado FFmpeg con carga dinámica de DLL para garantizar compatibilidad sin bloquear el inicio.
• Actualizados los filtros de descarga de podcasts para incluir los nuevos formatos de audio/video.
• Impedido que Ctrl+S guarde archivos de audio/video para evitar corrupción.
• Mejorada la importación de transcripciones de YouTube haciéndola más robusta y resiliente.
• Mejorada la robustez de la división en partes de audiolibros, asegurando que no se pierda texto.
• El instalador ahora es completamente multilingüe, soportando Italiano, Inglés, Español, Portugués, Sueco y Vietnamita según el idioma del sistema del usuario. El inglés es el predeterminado para sistemas no soportados.
• Categorías de podcasts: presionar Enter en una categoría ahora confirma la selección (equivalente al botón OK).
• Mejorado el sistema de detección de bloqueos para evitar falsos positivos cuando hay diálogos modales abiertos (mensajes de error, "texto no encontrado").
Correcciones
• Corregido un error donde el changelog no se abría al inicio.
• Corregido un error donde la solicitud de OCR no aparecía para PDFs no accesibles abiertos desde el Explorador.
• Corregido un error de inicio que podía causar pérdida de foco o cierre de ventanas inmediatamente después de abrir.
• Corregido un error crítico en la búsqueda regex que impedía encontrar texto, incluyendo problemas con "Búsqueda circular" y la opción "El punto equivale a nueva línea" con terminaciones de línea de Windows.
Localización
• Añadida la traducción al polaco.
• Añadida la traducción al francés.
• Añadida la traducción al checo (gracias a Radek Žalud y Jiri Holzinger).

Versión 0.6.1 – 2026-01-20
Correcciones
• Corregido un error por el cual, al activar “Mostrar voces en el editor” y reproducir un podcast, la reproducción se detenía.
• Corregido un problema por el cual algunos podcasts no podían añadirse mediante URL porque la dirección se truncaba.
• Corregido un error por el cual ya no era posible añadir URLs normales en la función de feeds RSS.
• Corregido un problema por el cual el idioma de Wikipedia se mostraba en varias pestañas de configuración.
• Eliminada la creación de algunos archivos de depuración que se generaban incluso en modo release.
Mejoras
• Mejorado el soporte para las voces de Microsoft, que ahora se reproducen mediante un método dedicado con un user agent diferente.
• Añadido soporte para archivos MP4.

Versión 0.6.0 – 2026-01-20
Nuevas funciones
• Añadido el corrector ortográfico. Desde el menú contextual es posible comprobar si la palabra actual es correcta y, en caso contrario, obtener sugerencias.
• Añadida la importación y exportación de podcasts mediante archivos OPML.
• Añadido soporte para la búsqueda en Podcast Index además de iTunes. El usuario puede introducir su API key y API secret gratuitos (generados usando solo su correo electrónico).
• Añadido soporte para voces SAPI4, tanto para la lectura en tiempo real como para la creación de audiolibros.
• Añadido un fallback automático de OCR para PDFs no accesibles: cuando no se encuentra texto extraíble, el documento se reconoce mediante OCR.
• Añadido soporte de diccionario mediante Wiktionary. Al pulsar la tecla Aplicaciones se muestran las definiciones y, cuando están disponibles, también sinónimos y traducciones a otros idiomas.
• Añadida la importación de artículos desde Wikipedia con búsqueda, selección de resultados e importación directa en el editor.
• Añadido el atajo Shift+Enter en el módulo RSS para abrir un artículo directamente en el sitio web original.
Mejoras
• La selección del micrófono ahora siempre es respetada por la aplicación.
• En la ventana de podcasts, al pulsar Enter sobre un episodio, NVDA anuncia inmediatamente “cargando”, proporcionando confirmación inmediata de la acción.
• En los resultados de búsqueda de podcasts, al pulsar Enter ahora se realiza la suscripción al podcast seleccionado.
• Corregidas y mejoradas las etiquetas de los atajos Ctrl+Shift+O y Podcast Ctrl+Shift+P.
• La velocidad de reproducción y el volumen ahora se guardan en la configuración y se mantienen para todos los archivos de audio.
• Añadida una carpeta de caché dedicada para los episodios de podcasts. El usuario puede conservar los episodios mediante “Conservar podcast” en el menú Reproducir. La caché se limpia automáticamente cuando supera el tamaño configurado por el usuario (Opciones → Audio).
• Mejorada de forma significativa la obtención de artículos RSS utilizando libcurl con impersonación de Chrome e iPhone, garantizando compatibilidad con aproximadamente el 99 % de los sitios.
• Añadido el estado leído / no leído para los artículos RSS, con indicación clara en la lista RSS.
• La función Reemplazar todo ahora muestra también el número de reemplazos realizados.
• Añadido el botón Eliminar podcast al navegar por la biblioteca de podcasts mediante Tab.
Correcciones
• Eliminada la entrada redundante “pending update” del menú Ayuda (las actualizaciones ya se gestionan automáticamente).
• Corregido un error por el cual, al abrir un archivo MP3 y pulsar Ctrl+S, el archivo se guardaba y quedaba corrupto.
• Corregido un problema de interfaz donde “Batch Audiobooks” se mostraba como “(B)… Ctrl+Shift+B” (se eliminó la etiqueta redundante).
• Corregido el funcionamiento de las comillas inteligentes: cuando están habilitadas, las comillas normales ahora se sustituyen correctamente por comillas tipográficas.
• Corregido un error por el cual, al usar “Ir al marcador”, la velocidad de reproducción se restablecía a 1.0.
• Corregido un problema por el cual los episodios de podcasts ya descargados se volvían a descargar en lugar de usar la versión en caché.
Atajos de teclado
• F1 ahora abre la guía.
• F2 ahora comprueba si hay actualizaciones.
• F7 / F8 ahora permiten desplazarse al error ortográfico anterior o siguiente.
• F9 / F10 ahora permiten cambiar rápidamente entre las voces guardadas en favoritos.
Mejoras para desarrolladores
• Los errores ya no se ignoran silenciosamente: se han eliminado todos los patrones let _ = y los errores ahora se gestionan explícitamente (propagados, registrados o tratados con mecanismos de respaldo adecuados).
• El proyecto ahora no compila si hay advertencias: tanto cargo check como cargo clippy deben completarse sin avisos, con lints más estrictos y eliminación de allow donde sea posible.
• Eliminadas las implementaciones personalizadas de tipo strlen / wcslen. Las longitudes de cadenas y buffers UTF-16 ahora se derivan de datos gestionados por Rust, sin escanear memoria manualmente.
• La gestión de DLL se ha limpiado y centralizado en torno a libloading, evitando lógica de carga personalizada y análisis PE.
• Eliminados los helpers manuales para el parsing de bytes: ahora todo el parsing utiliza from_le_bytes / from_be_bytes sobre slices verificadas.
Estos cambios reducen el uso innecesario de unsafe, eliminan posibles comportamientos indefinidos y hacen que el código sea más idiomático, robusto y mantenible.

Version 0.5.9 - 2026-01-13
Nuevas funciones
• Aniadida la posibilidad de reordenar RSS desde el menu contextual (arriba/abajo/a posicion) con controles para posiciones no validas.
• Aniadido un menu contextual para los articulos con abrir sitio original y compartir por WhatsApp, Facebook y X.
• Aniadido el atajo Esc para volver desde articulos importados a la lista RSS.
• Aniadido el modo podcast: buscar, suscribirse y escuchar; reordenar suscripciones; Esc detiene la reproduccion y vuelve a la lista; Enter en un episodio inicia la reproduccion.
• Aniadido el control de velocidad de reproduccion para podcasts y archivos MP3.
• Aniadido Ctrl+T para ir a un tiempo especifico.
• Aniadido un boton de vista previa de voz despues del combo de volumen.
• Aniadida la funcion de regex para Buscar y Reemplazar, estilo Notepad++.
• Aniadida la importacion de RSS desde archivos OPML y TXT.
• Aniadida la casilla en Opciones para habilitar "Abrir con Sonarpad" en el Explorador de archivos, tambien en version portable.
Mejoras
• Mejorada la seleccion de velocidad, tono y volumen de las voces, respetando los limites maximos del TTS.
• Varias mejoras de RSS para descargar todos los articulos sin mover el foco de NVDA durante las actualizaciones.
• Mejorada la reproduccion de audio con un menu dedicado, anuncio del tiempo con Ctrl+I y volumen hasta el 300%.
• Aniadidos atajos faltantes para algunas funciones.
• Reorganizado el menu Editar con un submenu para las funciones de limpieza de texto.
• Reorganizadas las Opciones en pestanas, con Ctrl+Tab y Ctrl+Shift+Tab para moverse entre ellas.
• Resueltos los problemas de lectura de articulos: el lector RSS ahora muestra los articulos completos como en el navegador.
Correcciones
• Corregido un problema por el que la limpieza de Markdown eliminaba numeros al inicio de linea.
• Corregido AltGr+Z que activaba Undo.
• Corregido un problema por el que al grabar un audiolibro no se podia detener rapidamente.
Localizacion
• Aniadida la traduccion vietnamita (gracias a Anh Duc Nguyen).

Version 0.5.8 - 2026-01-10
Nuevas funciones
• Aniadido control de volumen para microfono y audio del sistema al grabar podcasts.
• Aniadida una nueva funcion para importar articulos desde sitios web o feeds RSS, incluyendo los feeds mas importantes para cada idioma.
• Aniadida una funcion para eliminar todos los marcadores del archivo actual.
• Aniadida la funcion para eliminar lineas duplicadas y lineas duplicadas consecutivas.
• Aniadida la funcion para cerrar todas las pestanas o ventanas excepto la actual.
• Aniadida la entrada Donaciones en el menu Ayuda para todos los idiomas.
Mejoras
• Mejorado el terminal accesible para evitar algunos bloqueos.
• Mejoradas y corregidas las access key y los atajos de teclado del programa.
• Corregido un problema por el que al cerrar la ventana de reproduccion de audio la reproduccion no se detenia.
• Aniadidas ventanas de confirmacion para acciones importantes (p. ej., eliminar lineas duplicadas, eliminar guiones al final de linea, eliminar todos los marcadores del archivo actual). No se muestra confirmacion si la accion no se aplica.
• Aniadida la posibilidad de eliminar feeds/sitios RSS de la biblioteca seleccionandolos y pulsando Supr.
• Aniadido un menu contextual en la ventana RSS para modificar o eliminar feeds/sitios RSS.
• Eliminada la casilla para mover la configuracion a la carpeta actual; ahora el programa lo gestiona automaticamente (si la carpeta del exe se llama "sonarpad portable" o el exe esta en una unidad extraible, guarda en la carpeta del exe en `config`, si no en `%APPDATA%\\Sonarpad`, con fallback a `config` si la carpeta preferida no es escribible).

Version 0.5.7 - 2026-01-05
Nuevas funciones
• Aniadida opcion para grabar audiolibros en lote (conversion multiple de archivos y carpetas).
• Aniadido soporte para archivos Markdown (.md).
• Aniadida eleccion de codificacion al abrir archivos de texto.
• Aniadida opcion en el terminal para anunciar nuevas lineas con NVDA.
Mejoras
• La grabacion de audiolibros se guarda ahora en MP3 nativo cuando se selecciona.
• El usuario puede elegir donde colocar el asterisco * que indica cambios no guardados.
• Mejorado el sistema de actualizacion para ser mas robusto en diferentes escenarios.
• Aniadida en el menu Editar la funcion para eliminar guiones al final de linea (util para textos OCR).

Version 0.5.6 - 2026-01-04
Correcciones
  Mejorado Buscar en archivos: al pulsar Enter abre el archivo exactamente en el fragmento seleccionado.
Mejoras
  Soporte PPT/PPTX.
  Para formatos no textuales, Guardar ahora propone siempre .txt para no romper el formato (PDF/DOC/DOCX/EPUB/HTML/PPT/PPTX).
  Grabacion de podcast desde microfono y/o audio del sistema (menu Archivo, Ctrl+Shift+R).

Version 0.5.5 - 2026-01-03
Nuevas funciones
• Aniadido un terminal accesible optimizado para mucho output y lectores de pantalla (Ctrl+Shift+P).
• Aniadida la opcion de guardar la configuracion en la carpeta actual (modo portable).
Correcciones
• Mejorados los fragmentos de Buscar en archivos para que la vista previa quede alineada con la coincidencia.

Version 0.5.4 – 2026-01-03
Mejoras
• Correccion de Normalizar espacios en blanco (Ctrl+Shift+Enter).
• Soporte HTML/HTM (abrir como texto).

Version 0.5.3 – 2026-01-02
Nuevas funciones
• Se agrego Buscar en archivos.
• Se agregaron nuevas herramientas de texto: Normalizar espacios en blanco, Salto de linea duro y Quitar Markdown.
• Se agrego Estadisticas de texto (Alt+Y).
• Se agregaron nuevos comandos de lista en el menu Editar:
• Ordenar lineas (Alt+Shift+O)
• Eliminar duplicados (Alt+Shift+K)
• Invertir lineas (Alt+Shift+Z)
• Se agregaron Comentar / Descomentar lineas (Ctrl+Q / Ctrl+Shift+Q).
Localizacion
• Se agrego la localizacion en espanol.
• Se agrego la localizacion en portugues.
Mejoras
• Cuando un archivo EPUB esta abierto, Guardar cambia automaticamente a Guardar como y exporta el contenido como .txt para evitar la corrupcion del EPUB.

## 0.5.2 - 2026-01-01
- Se agrego un changelog.
- Se agregaron opciones "Abrir con Sonarpad" y asociaciones de archivos compatibles durante la instalacion.
- Se mejoro la localizacion de mensajes (errores, dialogos, exportacion de audiolibro).
- Se agrego la seleccion de partes al usar "Dividir audiolibro por texto", con la opcion "Requerir el marcador al inicio de la linea".
- Se agrego la importacion de transcripciones de YouTube con seleccion de idioma, opcion de marca de tiempo y mejoras de foco.

## 0.5.1 - 2025-12-31
- Actualizaciones automaticas con confirmacion, manejo de errores y notificaciones mejoradas.
- Mejoras en exportacion de audiolibros (division por texto, SAPI5/Media Foundation, controles avanzados).
- Mejoras en TTS (pausa/reanudar, diccionario de reemplazos, favoritos).
- Menu Ver y paneles de voces/favoritos, color y tamano del texto.
- Idioma predeterminado del sistema y mejoras de localizacion.
- CI y empaquetado Windows (artefactos, MSI/NSIS, cache).

## 0.5.0 - 2025-12-27
- Refactor modular (editor, manejo de archivos, menu, busqueda).
- Workflow de compilacion/empaquetado Windows y actualizaciones de README/licencia.
- Arreglo de navegacion TAB en la ventana de Ayuda.

## 0.5 - 2025-12-27
- Actualizacion preliminar del numero de version.

## 0.1.0 - 2025-12-25
- Version inicial: estructura del proyecto y README.

# Changelog

Versão 0.9.11 – 2026-09-22

Gravação de ficheiros
1. Corrigido Guardar como: quando um documento já aberto a partir do disco é guardado com um novo nome, a janela de gravação abre agora na mesma pasta do ficheiro original, em vez de começar na pasta predefinida configurada ou no OneDrive. Os novos documentos continuam a usar a pasta de gravação configurada.

Vozes Google
1. Foi adicionado um modo alternativo seguro para o arranque das vozes Google: se o Chrome fechar antes de o ambiente Google TTS estar pronto, o Sonarpad tenta automaticamente com o Microsoft Edge, quando disponível. Nos computadores onde as vozes Google já funcionam, o funcionamento atual com o Chrome permanece inalterado.

2. Melhorado o diagnóstico das raras falhas ao iniciar o navegador usado pelas vozes Google: stdout e stderr do Chrome ou Edge são escritos no diagnóstico do Sonarpad apenas quando o navegador termina antes de o runtime Google TTS estar pronto. Os arranques bem-sucedidos não adicionam a saída do navegador ao diagnóstico.

Versão 0.9.10 – 2026-09-16

Audiodescrições
1. Melhoradas as audiodescrições substituindo o caráter `_` (sublinhado) por um espaço, para evitar que a síntese de voz pronuncie esse caráter nas audiodescrições.

Versão 0.9.9 – 2026-09-15

Audiodescrição com IA
1. Adicionado um fallback isolado para vídeos de origem sem faixa de áudio, sem alterar a pipeline que já funciona. O Sonarpad continua a tentar primeiro a exportação normal; apenas se o FFmpeg indicar a ausência do fluxo de áudio e o Sonarpad confirmar que a origem não contém qualquer faixa de áudio é criada uma fonte silenciosa com a mesma duração, sobre a qual são misturadas as descrições sintetizadas. Ao exportar para o vídeo original, é depois reutilizado o caminho de mux vídeo+áudio já existente. Vídeos que já têm áudio continuam exatamente pelo caminho normal.

2. Expandido o fallback estritamente isolado para a ausência de descrições sem alterar a pipeline normal de audiodescrição. O Sonarpad continua a executar primeiro a análise existente; apenas quando termina sem descrições utilizáveis oferece uma segunda tentativa em modo Breve, usando a mesma pipeline e mantendo todas as restrições de silêncio e de não sobreposição com diálogos. Se essa segunda tentativa também não conseguir inserir uma descrição, o Sonarpad pede agora autorização explícita para um último fallback. Só depois de escolher Sim reutiliza as descrições breves já geradas pelo Gemini e guardadas no checkpoint da segunda tentativa, misturando-as nos respetivos tempos visuais com ducking e permitindo breves sobreposições com os diálogos. Pyannote, a bridge Gemini, as regras normais de alinhamento, o agendamento TTS e os dois primeiros percursos de análise não são alterados. Se o utilizador recusar, ou não existirem descrições geradas para reutilizar, o Sonarpad termina corretamente com uma mensagem localizada.

3. Aperfeiçoado o fallback isolado quando não restam descrições utilizáveis, sem alterar o fluxo normal de audiodescrição. Se a verbosidade escolhida inicialmente já for Breve e a análise terminar sem descrições utilizáveis, o Sonarpad passa a ignorar a segunda análise Gemini redundante e, quando existem descrições geradas reutilizáveis, pergunta diretamente se deve usar o fallback final com sobreposição de diálogo. Se a verbosidade inicial for Normal ou Detalhada, a tentativa Breve é mantida mesmo quando as pausas disponíveis são muito curtas: nesse caso também serve para gerar uma descrição mais curta para o eventual fallback final, reduzindo o tempo em que a narração se sobrepõe ao diálogo. Pyannote, o bridge Gemini e as regras normais de silêncio e alinhamento permanecem inalterados.

Gravação de podcasts
1. Adicionado um fallback conservador para a posição do dispositivo do microfone em alguns controladores WASAPI problemáticos, sem alterar a gravação nos sistemas que já funcionam. Só é ativado após incompatibilidades persistentes de posição: 24 consecutivas ou pelo menos 80% em 64 pacotes limpos. Os verdadeiros `DATA_DISCONTINUITY` do WASAPI e os erros de timestamp continuam a ter prioridade, e a captura do áudio do sistema permanece inalterada.

YouTube e streaming
1. Corrigidos os downloads do YouTube que falhavam quando iniciados pelo menu de contexto dos resultados. Os erros conhecidos do yt-dlp para vídeos exclusivos de membros do canal são agora convertidos na mensagem localizada do Sonarpad em vez de mostrar o texto bruto em inglês. Depois de fechar o erro, o Sonarpad restaura o foco apenas quando o ciclo nativo do menu de contexto/modal termina realmente, regressando de forma fiável à lista de resultados com o teclado ativo. A pipeline normal dos downloads bem-sucedidos não é alterada.

2. O filtro preventivo foi alinhado com o SonarTube móvel: nos resultados de pesquisa e na navegação por canais/listas de reprodução, são agora ocultados os vídeos para os quais o YouTube/yt-dlp não devolve dados de visualizações. Canais e listas continuam visíveis e vídeos reais com 0 visualizações são mantidos. O tratamento localizado do erro de conteúdo exclusivo para membros permanece como segundo fallback. O download múltiplo de listas não é alterado.

Versão 0.9.8 – 2026-09-12

Audiodescrição com IA
1. Foi adicionada uma recuperação conservadora para exportação multicanal sem alterar o pipeline dos ficheiros que já funcionam. O Sonarpad continua a usar primeiro a disposição de canais original e só completa um último quadro PCM incompleto quando necessário. Se o WAV intermédio multicanal ainda falhar porque o número de amostras não é múltiplo do número de canais, o Sonarpad repete automaticamente apenas essa exportação em estéreo. As exportações mono, estéreo e multicanal que já funcionam permanecem inalteradas.

Tema escuro e acessibilidade
1. Corrigido um problema em que o tema escuro interferia com diálogos nativos do Windows e mensagens de confirmação. O Sonarpad deixa de aplicar o seu subclass personalizado de tema escuro às árvores de diálogo do sistema `#32770`, mantendo Abrir/Guardar, a exportação de diagnósticos e MessageBox sob gestão do Windows e com foco correto do teclado/NVDA. Também foi corrigido o tempo de vida da extensão ZIP predefinida no diálogo Guardar dos diagnósticos.

Versão 0.9.7 – 2026-09-11

Audiodescrição com IA
1. Adicionada a opção facultativa para criar a audiodescrição no ficheiro de vídeo original. Quando ativa, o Sonarpad passa a preferir MP4: copia o fluxo de vídeo original sem recodificação e adiciona a faixa de audiodescrição misturada. Se o FFmpeg indicar uma incompatibilidade do contentor ou da escrita de pacotes ao criar o MP4, o Sonarpad repete automaticamente a mesma exportação em MKV, também sem recodificar o vídeo. Também é possível escolher MKV explicitamente. Neste modo, as pausas prolongadas são ignoradas para manter áudio e vídeo sincronizados.

2. Melhorada a reanálise de segmentos nos projetos de audiodescrição guardados. A janela passa a usar o acesso à IA já configurado na janela principal Criar audiodescrição, deixando de duplicar a chave API, o crédito Sonarpad AI e a escolha do modelo. Um segmento reanalisado passa a manter todo o grupo de descrições do projeto pertencentes ao mesmo segmento de análise, em vez de apenas a primeira ou a mais próxima; todas as descrições do grupo ficam marcadas como reanalisadas e “Aplicar segmento e reexportar MP3” aplica o grupo inteiro numa única operação. As pré-visualizações das pausas prolongadas passam a ser sintetizadas diretamente em vez de procurar a posição no MP3 exportado, evitando que a pré-visualização comece com áudio do filme ou corte a descrição.

3. Foi melhorado o fallback conservador de multimédia do Gemini sem alterar o fluxo que já funciona. O HTTP 400 `INVALID_ARGUMENT` continua a ativar segmentos MKV mais pequenos (cerca de 15 MB) e depois segmentos MP4 compatíveis. Além disso, se o Gemini aceitar um segmento enviado mas o processamento terminar em `FAILED`, o Sonarpad volta a enviar exatamente o mesmo segmento uma vez e só depois passa para MP4; as tentativas do Code 13 do servidor ficam limitadas a três antes do mesmo fallback, evitando esperas infinitas. Erros de rede, quota, autenticação ou síntese não ativam este caminho.

4. Corrigida a persistência das credenciais de IA ao mudar o modo de acesso. A chave API pessoal do Gemini e o código de acesso do Sonarpad AI passam a ser guardados de forma independente: alternar entre Chave API pessoal e Sonarpad AI já não apaga a credencial inativa. A chave Gemini também é guardada quando o respetivo campo perde o foco.

5. Foi adicionado um verdadeiro botão Cancelar a “Reanalisar segmento” e “Aplicar segmento e reexportar MP3”. A barra de progresso continua ativa e Cancelar passa a interromper a preparação do segmento pelo FFmpeg, o worker de IA, a validação TTS e a exportação final assim que a fase em curso puder parar em segurança. A aplicação de um segmento reanalisado é agora transacional: o projeto e o ficheiro multimédia existentes só são substituídos depois de uma reexportação concluída com êxito; se a operação for cancelada, os ficheiros anteriores permanecem inalterados e a proposta reanalisada continua disponível para nova tentativa.

6. Corrigida a reanálise de segmentos para descrições fora do primeiro chunk do Gemini. O segmento isolado é agora enviado ao worker existente com uma linha temporal local iniciada em 0 e depois remapeado para a posição absoluta no projeto, evitando `Invalid prepared chunk timeline at chunk 1` sem alterar o fluxo normal de audiodescrição.

7. A reanálise de segmentos em projetos guardados foi refeita para usar os mesmos controlos de segurança da criação completa de audiodescrição. O segmento selecionado passa agora pela mesma extração de áudio mono a 16 kHz e análise de diálogos/silêncios com Pyannote, pelas mesmas regras temporais e fallbacks do Gemini, pela mesma síntese TTS real com a voz do projeto e pelo mesmo agendador final para colocação segura e pausas prolongadas. Ao aplicar o segmento, o Sonarpad mantém os novos tempos absolutos já verificados e atualiza a proteção Pyannote desse segmento; foi removida a solução especial anterior da pré-visualização para textos reanalisados demasiado longos.

8. Corrigida a substituição estrutural do segmento após a reanálise completa. O Sonarpad já não exige que o segmento novamente verificado tenha exatamente o mesmo número de descrições do projeto guardado: as descrições antigas desse chunk são substituídas pelo resultado seguro completo da pipeline, pelo que um segmento com 10 descrições pode passar corretamente para 9 (ou 11). O editor mostra imediatamente a nova quantidade e os novos tempos para pré-visualização, enquanto o projeto e o ficheiro multimédia guardados permanecem inalterados até “Aplicar segmento e reexportar MP3” terminar com sucesso.

9. A reanálise de segmentos foi reformulada para tratar o trecho físico selecionado como um verdadeiro mini-filme autónomo. O Sonarpad passa agora esse mini-filme diretamente para exatamente a mesma função `create_audio_description` usada na criação completa normal, para que vídeo e faixa de áudio partilhem a mesma linha temporal local e os controlos normais de Pyannote, Gemini, TTS real, agendamento e segurança sejam executados sem uma implementação separada de reanálise. Só depois de terminar esse fluxo normal é que os tempos locais resultantes são deslocados de volta para a posição original no filme.

Edição de texto
1. Melhorado “Unir linhas quebradas” em Editar > Texto. Sem seleção, processa o documento inteiro; com uma seleção, atua apenas nas linhas selecionadas. Agora reconstrói parágrafos completos ao unir linhas consecutivas mesmo quando uma frase termina com pontuação; as linhas em branco continuam a separar parágrafos e as listas numeradas ou com marcadores são preservadas.

10. Corrigida a sincronização temporal da reanálise ao mapear o mini-filme analisado de volta para o filme original. O Sonarpad aplica agora a mesma pequena reconciliação da duração dos segmentos usada na análise completa às descrições, aos tempos de Visual Evidence e aos intervalos de diálogo protegidos, evitando desvios progressivos de frações de segundo sobre a fala.

Versão 0.9.6 – 2026-09-09

1. Corrigidos os bloqueios de algumas vozes SAPI4 com o acompanhamento do cursor ativo. O bridge aguarda agora a conclusão efetiva da síntese, mantendo o acompanhamento ativo.

2. Corrigidos a sincronização entre microfone e áudio do sistema e os estalidos causados pelo arredondamento dos carimbos de tempo na gravação de podcasts. As correções também se aplicam ao guardar as fontes separadamente.

3. A síntese SAPI5 das audiodescrições utiliza agora processos separados. Em caso de falha, repete a síntese com menos paralelismo, conserva as descrições concluídas e memoriza o limite funcional para a voz.

Versão 0.9.5 – 2026-09-07

Audiodescrição com IA
1. Melhorada a resiliência quando o Gemini devolve temporariamente `MALFORMED_RESPONSE` sem conteúdo utilizável. O Sonarpad passa agora a repetir exatamente o mesmo segmento até três vezes, com uma breve espera entre tentativas, em vez de interromper imediatamente toda a audiodescrição. As respostas normais do Gemini, os bloqueios de segurança, os erros de limite de tokens e o comportamento atual dos pontos de controlo permanecem inalterados.

2. Corrigido outro caso raro de `invalid Gemini chunk timeline` em alguns vídeos cujos segmentos preparados pelo FFmpeg apresentam um pequeno desvio acumulado de duração. O Sonarpad mantém o percurso normal inalterado quando a linha temporal é válida e utiliza um reajuste conservador apenas quando a linha temporal normal falharia e a diferença total é pequena; diferenças grandes ou suspeitas continuam a produzir um erro.

3. Adicionado o serviço opcional Sonarpad AI para audiodescrições com IA. Os utilizadores podem continuar a usar a sua própria chave API Gemini ou escolher o serviço Sonarpad e introduzir um código Sonarpad pessoal. O código é protegido com Windows DPAPI. No modo de serviço, os segmentos de vídeo preparados são enviados diretamente do PC do utilizador para a Google através de um URL temporário de carregamento da Google; sonarpad.com não recebe nem guarda os bytes do vídeo. O backend apenas autoriza o acesso, impõe o modelo Gemini/nível Standard e os limites de utilização, contabiliza o consumo e devolve a resposta do Gemini.

4. Melhorada a edição de projetos de audiodescrição guardados. Agora é possível alterar várias descrições seguidas sem ter de aplicar cada alteração imediatamente. As alterações ficam em memória; ao premir “Aplicar texto”, o Sonarpad verifica cada descrição modificada em relação ao respetivo intervalo guardado e depois guarda em conjunto todas as alterações válidas. Se uma descrição não couber no seu intervalo, o foco volta a essa descrição sem perder as outras alterações pendentes.

5. Adicionado “Ajustar voz” imediatamente antes de “Criar audiodescrição”. A nova janela permite escolher motor, voz, velocidade e volume e inclui “Testar voz”. Ao premir OK, o foco volta exatamente a “Ajustar voz” e os valores escolhidos são aplicados à audiodescrição que será criada. Se a janela não for utilizada, a síntese mantém-se exatamente como antes.


6. Foram ampliados os controlos de voz nos projetos de audiodescrição guardados. Em Modificar projeto, além do motor e da voz, estão agora disponíveis velocidade, volume e “Testar voz”. Ao aplicar os parâmetros de voz, o Sonarpad volta a verificar todas as descrições existentes com os novos parâmetros de síntese e só reconstrói o MP3 se todas continuarem a caber nos respetivos intervalos guardados. Os intervalos sem diálogo, os tempos de Visual Evidence, as descrições Gemini e a análise temporal do projeto são reutilizados sem executar novamente a análise de IA.

Versão 0.9.4 – 2026-09-04

Audiodescrição com IA
1. Corrigido um problema com vídeos Matroska/WebM que contêm uma marca temporal inicial interna invulgarmente elevada e que podia interromper a geração da audiodescrição com o erro “invalid Gemini chunk timeline”. O Sonarpad passa agora a normalizar a duração do ficheiro de origem e dos segmentos Gemini apenas quando este padrão anómalo de marcas temporais é detetado, mantendo inalterados os vídeos normais e o comportamento da audiodescrição que já funcionava.
2. Melhorado o ducking da audiodescrição para obter uma mistura mais natural. O áudio original passa agora a baixar suavemente antes de a voz começar e regressa de forma gradual ao nível normal depois da descrição; quando existem descrições muito próximas, o nível de fundo mantém-se estável, evitando sucessivas descidas e subidas de volume.
3. Corrigido um problema que podia fazer falhar a exportação final da audiodescrição com alguns ficheiros AVI e outras fontes com áudio 5.1/multicanal, quando a descodificação terminava com um quadro de áudio intercalado incompleto. O Sonarpad passa agora a completar em segurança apenas esse último quadro parcial com o número estritamente necessário de amostras silenciosas; os quadros já completos e os ficheiros mono, estéreo ou multicanal normais permanecem inalterados.


Versão 0.9.3 – 2026-09-03

Vozes SAPI5
1. Corrigido um problema em que algumas vozes SAPI5 locais podiam não falar com o movimento do cursor ativado, durante a leitura com várias vozes ou ao criar audiolivros/MP3. O Sonarpad usa agora um caminho de síntese SAPI5 para ficheiro que funciona de forma fiável no Windows e continua a permitir cancelar a síntese, enquanto a reprodução direta normal permanece inalterada.
2. Corrigida a posição do cursor durante a leitura de diálogos com várias vozes. As etiquetas de voz inseridas automaticamente pelo Sonarpad para os diálogos passam a ser tratadas apenas como metadados de reprodução e não como caracteres no editor, evitando que após F4 ou F6 o cursor avance para além do texto real. A leitura com uma única voz e as etiquetas <voice> escritas explicitamente nos documentos permanecem inalteradas.

Audiodescrição com IA
1. Adicionada a caixa “Mostrar chave API” imediatamente após o campo da chave API Gemini. Está desativada por predefinição; quando ativada, mostra temporariamente a chave completa para permitir confirmar que foi colada por inteiro. Ao reabrir a janela, a chave volta sempre a ficar oculta.

Podcasts e Wikipédia
1. Depois de guardar uma gravação de podcast, o Sonarpad pergunta agora se pretende abrir a pasta que contém o ficheiro guardado, tal como já acontece ao guardar conteúdos do YouTube/streaming.
2. Ao importar outro artigo da Wikipédia para um editor que já contém texto, o novo artigo passa a ser acrescentado no fim em vez de ser inserido no início. O cursor é colocado no início do artigo recém-importado.


Versão 0.9.2 – 2026-09-02

Audiodescrição com IA
1. Corrigido um problema que podia fazer a audiodescrição com IA falhar durante a exportação final para MP3 em vídeos com áudio multicanal, como 5.1. O Sonarpad agora converte automaticamente o áudio multicanal para estéreo apenas quando necessário para a codificação MP3, sem alterar as exportações mono ou estéreo.
2. Ao iniciar a Audiodescrição com IA num vídeo com várias faixas de áudio, o Sonarpad passa a perguntar qual faixa deve ser utilizada antes do processamento. A caixa de combinação acessível pode ser alterada com as setas; OK inicia a audiodescrição com a faixa selecionada, enquanto Cancelar fecha a janela de audiodescrição e devolve o foco ao editor do Sonarpad.

YouTube e streaming
1. Corrigido um problema em que iniciar a audiodescrição com IA num vídeo da página 2 ou posterior de uma lista de reprodução ou canal do YouTube podia reabrir a janela de seleção do YouTube e retirar o foco da janela de audiodescrição. O Sonarpad fecha agora corretamente o seletor sem voltar às páginas anteriores.
Versão 0.9.1 – 2026-09-01

Transferências do YouTube
• Corrigido um problema que podia fazer com que as janelas de progresso das transferências do YouTube/streaming regressassem repetidamente ao primeiro plano depois de mudar para outra aplicação com Alt+Tab. As transferências agora continuam em segundo plano sem roubar o foco.
• Melhorada a acessibilidade do progresso das transferências. Ao regressar à janela de progresso, os leitores de ecrã podem ler o estado atual e a percentagem. Nas listas de reprodução, o Sonarpad também indica o número do item atual, o total de itens e o título.
• Corrigidos falsos avisos de bloqueio do watchdog durante transferências e conversões longas quando a janela de progresso continuava a responder.
• Foi adicionada uma caixa de combinação Formato aos downloads de playlists. Na lista de vídeos, prima Tab para escolher MP4, MP3, M4A, OPUS, OGG, WAV ou FLAC antes de iniciar o download múltiplo.
• O processo de guardar multimédia em streaming foi reorganizado. O formato e a qualidade passam a ser escolhidos no momento de guardar, e não na janela inicial de pesquisa de streaming. “Guardar multimédia” abre uma única janela de Formato e Qualidade e os downloads de playlists disponibilizam ambas as caixas de combinação.

Audiodescrição com IA
• Corrigido um problema que podia impedir o início da audiodescrição com IA em alguns vídeos MKV. O Sonarpad agora lida de forma mais fiável com vídeos que têm marcas de tempo irregulares ou em falta.

Versão 0.9.0 – 2026-08-31

Audiodescrição com IA — nova função principal
• Foi adicionada a opção «Criar audiodescrição com IA» em Ferramentas > Multimédia. O Sonarpad analisa o áudio para encontrar espaços sem diálogo, gera as descrições com Gemini e utiliza os motores de voz já disponíveis, evitando falar por cima dos diálogos.
• Foi melhorada a sincronização entre o que acontece no vídeo e as descrições, com verificações automáticas dos tempos gerados pelo Gemini.
• «Ativar pausas prolongadas» está desmarcada por predefinição. Pode ser ativada em conteúdos com muitos diálogos ou pouco espaço disponível para permitir descrições mais longas.
• O Sonarpad pode tentar reconhecer as personagens e utilizar os seus nomes. Os catálogos de personagens podem ser mantidos entre episódios de uma série para melhorar a continuidade.
• É possível guardar o projeto, editar posteriormente as descrições e voltar a exportar sem ter de gerar tudo novamente com Gemini.
• Se o processo for interrompido, o Sonarpad conserva o progresso e permite continuar a audiodescrição. Se a quota do Gemini se esgotar, é possível esperar, mudar de modelo ou interromper sem perder o trabalho já concluído.
• A janela permite escolher idioma, nível de detalhe, modelo Gemini, motor e voz, e memoriza as preferências utilizadas.
• O módulo está disponível nos 17 idiomas do Sonarpad. Durante a geração, a interface mostra apenas o progresso, o estado atual e Cancelar; no final, o MP3 pode ser aberto diretamente no leitor interno.

Livros eletrónicos e documentos
• Foi adicionado o import de Kindle sem DRM nos formatos MOBI, AZW e AZW3, com texto e capítulos disponíveis no editor e no índice.
• Foi adicionado suporte para DAISY 2.02 e DAISY 3. Os audiolivros DAISY usam o leitor interno do Sonarpad e respeitam a navegação e os limites dos capítulos.
• Kindle e DAISY são importados sem substituir o ficheiro original; os Kindle protegidos por DRM são recusados explicitamente.
• Foi corrigido «Guardar como» para EPUB: ao escolher TXT ou outro formato, é agora usada a extensão selecionada e o EPUB original permanece associado ao documento aberto.

RSS e artigos
• Foi adicionada a seleção múltipla de artigos RSS para eliminar vários numa única operação.
• Os RSS suportam agora pastas reais, preservadas na importação e exportação OPML, incluindo pastas vazias.
• Os feeds podem ser reordenados dentro da pasta atual com Mover para cima, Mover para baixo, Mover para o início, Mover para o fim e Mover para a posição.

Acessibilidade, guias e interface
• Os guias do Sonarpad foram reorganizados com um índice e foi adicionado um guia completo sobre Audiodescrição com IA.
• Foi corrigido um problema da tradução alemã que podia impedir a apresentação de Abrir, Guardar como e outras janelas de seleção de ficheiros.

Vozes e idiomas
• O catálogo descarregável Google TTS passou de 104 para 156 pacotes e de 53 para 81 variantes linguísticas.
• Foram adicionados novos pacotes Google TTS e nomes localizados para mais idiomas em toda a interface.

Versão 0.8.4 – 2026-07-24

Edição de documentos EPUB
• O Sonarpad consegue agora não só abrir documentos EPUB, mas também editá-los e voltar a guardá-los em formato EPUB, preservando a formatação original, o índice, as notas de rodapé, as imagens, as folhas de estilo, os metadados e as ligações internas.
• O formato EPUB está disponível em «Guardar como» para documentos abertos a partir de um EPUB. Ao guardar, apenas o texto alterado é atualizado e a estrutura do livro permanece intacta.

Fiabilidade dos audiolivros
• Corrigido um problema intermitente em que, após cinco tentativas falhados do Google TTS, uma unidade de síntese era descartada silenciosamente e podia faltar uma parte do texto no audiolivro final.
• As unidades Google são agora repetidas até terem êxito ou até o utilizador cancelar. O arranque dos processos é escalonado para reduzir conflitos temporários com o Chrome e os ficheiros; o Sonarpad também interrompe a criação em vez de guardar um audiolivro com um segmento em falta.
• Os audiolivros Edge passam a repetir sem limite fixo os erros temporários de rede, WebSocket, tempo limite, limitação do serviço e áudio inválido, até obter êxito ou o utilizador cancelar, incluindo vozes mistas e divisão por duração. SAPI4 e SAPI5 mantêm tentativas adaptativas e limitadas; se um segmento continuar a falhar, o Sonarpad interrompe a operação sem guardar um audiolivro incompleto.

Navegação nas bibliotecas digitais
• Os resultados do LibriVox, Internet Archive e Project Gutenberg utilizam agora navegação por páginas como o YouTube: “Ir para os resultados anteriores” aparece no início da lista e “Ir para os próximos resultados” no fim.
• Foram corrigidas as transições de foco no LibriVox: ao abrir um livro ou capítulo, o foco do NVDA já não passa para o editor principal antes de abrir a lista seguinte ou o leitor.
• Foi adicionada uma proteção do foco durante as pesquisas e o carregamento de livros do LibriVox: uma janela de carregamento localizada permanece em primeiro plano durante todo o pedido, impedindo que o foco do NVDA passe para a Linha de Comandos, o Windows Terminal ou outra aplicação.

Transferência de listas de reprodução do YouTube
• Foi adicionado às listas de reprodução do YouTube um comando acessível de seleção múltipla, que permite escolher os vídeos a descarregar sem alterar o comando existente “Guardar multimédia” para o elemento em reprodução.
• Os itens selecionados são descarregados um de cada vez com o formato e a qualidade escolhidos ao abrir a lista, recebem nomes numerados que preservam a ordem original e são guardados numa pasta própria dentro da pasta Multimédia configurada.
• A janela inclui “Selecionar tudo” e “Desmarcar tudo”, anuncia o número de itens selecionados, permite cancelar mantendo os ficheiros já concluídos e indica claramente os itens que não foi possível descarregar.
• Os elementos da lista de reprodução são agora caixas de verificação nativas: os leitores de ecrã anunciam automaticamente o título, o tipo de controlo e o estado marcado ou desmarcado, sem acrescentar palavras ao título visível nem utilizar anúncios de voz forçados.

Versão 0.8.3 – 2026-07-23

Modo escuro
• Adicionado um modo escuro, que pode ser ativado no menu Ver e fica guardado nas preferências.
• O tema escuro é aplicado ao editor, aos menus, às janelas secundárias e aos principais controlos, adaptando as cores do texto para manter a legibilidade e a acessibilidade.

Idioma alemão
• Adicionado o alemão como idioma completo da interface, selecionável nas Opções.
• Notícias e RSS, corretor ortográfico, calendário e todas as citações, donativos, guia e changelog estão integralmente disponíveis em alemão.

Português do Brasil e Google Notícias
• Adicionado o Português (Brasil) como idioma completo da interface, separado do Português (Portugal) e selecionável nas Opções.
• A interface, o calendário e todas as citações, o corretor ortográfico, as doações, o guia e o registo de alterações estão integralmente disponíveis em português brasileiro.
• O Google Notícias suporta agora a localização brasileira, as categorias do Brasil e fontes RSS brasileiras predefinidas separadas.
• Quando o feed as fornece, as fontes do Google Notícias relacionadas com a mesma notícia são apresentadas como itens filhos acessíveis na árvore.

LibriVox
• A pesquisa do LibriVox foi otimizada para evitar pedidos excessivos ao serviço e bloqueios da interface. Foram removidas as pesquisas extensas no catálogo, reduzidas as tentativas e introduzidos tempos limite mais curtos.

Síntese de voz
• As sequências de três ou mais pontos são agora normalizadas antes da leitura, evitando que algumas vozes pronunciem «ponto ponto» ou criem segmentos formados apenas por pontuação.

Artigos relacionados do Google Notícias
• Para cada notícia, quando disponíveis, são agora apresentados artigos relacionados, ou seja, outros artigos que tratam da mesma notícia. Para os ler, basta expandir o artigo principal quando o Sonarpad indicar que existem artigos relacionados disponíveis. Quem não quiser expandir esta secção só precisa de premir Enter no artigo principal e ler a notícia como sempre.
• Os artigos relacionados utilizam agora o mesmo sistema lido/não lido dos artigos principais, incluindo anúncios acessíveis, data e hora, gravação do estado e a sua conservação após a atualização das fontes ou o reinício do Sonarpad.

Anúncios nas partes dos audiolivros
• Foi adicionada às Opções de áudio a caixa de combinação «Anúncio no início de cada parte». Nos audiolivros divididos em vários ficheiros, cada parte pode começar sem anúncio, com o título do livro, o título e o número da parte, o nome do ficheiro ou o nome do ficheiro e o número da parte.

Versão 0.8.2 – 2026-07-17

Bibliotecas digitais e audiolivros
• Adicionado o Project Gutenberg, com pesquisa por título ou autor e seleção do idioma.
• Os livros EPUB do Project Gutenberg são descarregados para Documentos\Sonarpad\Documents; no fim, o Sonarpad pergunta se o livro deve ser aberto imediatamente no editor.
• Adicionado o Internet Archive para pesquisar e ouvir coleções de áudio, incluindo programas de rádio antigos, discursos e música ao vivo.
• Adicionado o LibriVox para pesquisar audiolivros por título ou autor e reproduzir diretamente os capítulos com o mesmo leitor utilizado para podcasts.
• As três novas funções estão disponíveis no menu Ferramentas e, quando o agrupamento dos menus está ativo, na secção Leitura.

Transcrições de áudio longas
• Corrigida a transcrição de ficheiros de áudio longos: o áudio é agora dividido automaticamente em partes de 15 minutos, transcrito uma parte de cada vez e depois reunido, evitando erros que podiam ocorrer com gravações longas.

YouTube
• As ações mais úteis que anteriormente só estavam disponíveis depois de abrir um vídeo do YouTube e aceder ao menu Reprodução estão agora também disponíveis diretamente no menu de contexto do mesmo vídeo, como «Transcrever áudio atual», «Criar audiodescrição com IA» e «Guardar mídia», para uma utilização mais simples.
• Adicionada a opção “Copiar ligação”, também disponível com Ctrl+C, para copiar para a área de transferência o URL do vídeo, da lista de reprodução ou do canal do YouTube selecionado.

Versão 0.8.1 – 2026-07-16

Síntese de voz Google
• Corrigido o arranque do Google TTS em sistemas Windows nos quais as ligações aceites pelo servidor interno do navegador herdavam o modo de socket não bloqueante, causando o erro 10035 e impedindo as vozes descarregadas de falar.
• O Sonarpad aguarda agora que o motor WASM do Chrome ou Edge esteja totalmente carregado antes da pré-visualização da voz ou da leitura com F5, evitando o erro “Chrome WASM TTS engine was not loaded”.
• O navegador oculto desativa a tradução de páginas e a acessibilidade do processo de renderização, evitando anúncios como “Traduzir página” e interferências com os comandos de leitura.
• O painel «Vozes no editor» mostra agora o botão «Gerir vozes Google...» quando o motor Google está selecionado e atualiza imediatamente a lista de vozes instaladas ao fechar o gestor.
• Os avisos de dependências apresentados ao remover pacotes de voz Google estão agora traduzidos em todos os idiomas da interface.

Experiência de atualização
• Após uma atualização automática, a janela de conclusão com o registo de alterações abre depois da reposição inicial do foco e permanece em primeiro plano, em vez de aparecer apenas depois de premir Tab.

Documentos PDF
• Corrigidos os PDF cujo texto incorporado continha caracteres NUL e era truncado na primeira ocorrência ao ser carregado no editor.
• Quando o pdf-extract devolve caracteres NUL incorporados, o Sonarpad tenta novamente com PDFium; quaisquer NUL restantes são removidos antes de enviar o texto aos controlos do Windows, preservando o resto do documento.

Acessibilidade dos menus
• Foi removido o cálculo de mnemónicas durante a execução: as teclas de acesso estão agora escritas explicitamente em cada uma das 15 traduções da interface e permanecem iguais em todos os arranques.
• Foram revistas todas as entradas estáveis dos menus principais e submenus, incluindo Reprodução, tipos de letra, Guardar imagem e Mostrar índice EPUB; mnemónicas em falta ou duplicadas entre itens do mesmo nível foram corrigidas diretamente nas traduções.
• Os testes automáticos passam apenas a validar as traduções e falham se uma mnemónica estiver em falta, for inválida ou estiver duplicada; já não alteram os rótulos durante a execução.
• Em menus excecionalmente extensos, quando o texto traduzido não fornece caracteres distintos suficientes, é apresentada uma tecla de acesso numérica explícita no formato padrão do Windows «(&1)».

Versão 0.8.0 – 2026-07-15

Dicionário online
• Adicionado o alemão ao dicionário online Wiktionary.
• As definições e os sinónimos em alemão são agora reconhecidos corretamente de acordo com a estrutura específica do Wiktionary alemão.

Confiabilidade dos audiolivros SAPI5
• A criação de audiolivros SAPI5 continua usando até 12 workers em paralelo quando a voz selecionada produz resultados confiáveis.
• Cada parte é verificada pelo tamanho do arquivo, duração estimada e uma comparação prudente com o texto atribuído.
• Partes ausentes ou suspeitas são geradas novamente com redução progressiva da concorrência: 12, 8, 6, 4, 2 e por fim 1 worker. Apenas as partes problemáticas são repetidas.
• O limite confiável é lembrado separadamente para cada voz SAPI5, sem desacelerar as vozes que funcionam corretamente com 12 workers.
• Uma verificação final impede que um MP3 muito mais curto que as partes geradas seja aceito silenciosamente.
• Os detalhes são gravados em `sapi5_audiobook_diagnostic.log`.
• Cada unidade de síntese SAPI5 passa a ser executada num processo Sonarpad separado e invisível. Se uma voz de terceiros falhar, apenas esse worker é encerrado e a aplicação principal permanece aberta.
• Durante a mesma criação do audiolivro, as partes não concluídas são imediatamente repetidas com o nível de concorrência inferior seguinte; as partes já validadas são preservadas.
• A recuperação no arranque seguinte permanece como proteção adicional apenas se a aplicação principal ou o computador forem interrompidos.

Processos de audiolivros SAPI4
• O número de processos SAPI4 escolhido pelo utilizador passa a ser respeitado até ao máximo técnico de 64; o limite oculto anterior de 16 foi removido.
• O número efetivo só é reduzido quando o audiolivro contém menos unidades de trabalho do que o solicitado.
• Se um ou mais processos da ponte SAPI4 falharem, as partes concluídas são preservadas e apenas as unidades com falha são repetidas automaticamente com concorrência progressivamente menor.
• O Sonarpad verifica agora o código de saída da ponte SAPI4 e rejeita partes de áudio vazias ou inválidas.

Configuração do proxy
• Foi adicionado um campo separado para a porta do proxy nas definições de rede.
• A porta pode ser indicada independentemente do endereço, é validada entre 1 e 65535 e substitui corretamente uma porta já presente no URL.

Pesquisa de rádio por idioma e país
• Os filtros Idioma e País passam a ser atualizados com todas as opções disponíveis no catálogo Radio Browser e deixam de estar limitados a uma lista fixa.
• Os nomes dos idiomas passam a ser reconhecidos mesmo quando o Radio Browser os fornece noutro alfabeto, na forma nativa, como abreviaturas ou como combinações de vários idiomas, sendo apresentados traduzidos no idioma atual da interface. Os valores que não representam idiomas reais, como números, géneros musicais, países ou descrições genéricas, são filtrados.
• O catálogo é atualizado em segundo plano e mantém uma lista alternativa utilizável quando o Radio Browser não está acessível.
• As entradas de idioma do Radio Browser que ficam idênticas após a tradução são agora reunidas num único item da lista, evitando passos silenciosos com leitores de ecrã.

Melhoria principal: sincronização entre a leitura e o cursor
• A sincronização entre a leitura por voz e o movimento do cursor foi significativamente melhorada para todos os motores de voz suportados.
• Quando a opção “Mover cursor durante a leitura” está ativa, o Sonarpad utiliza agora um sistema de progresso comum para Microsoft Edge Neural, Google TTS, SAPI4, SAPI5 e OneCore.
• O cursor acompanha com maior precisão o texto efetivamente pronunciado, com uma divisão mais coerente das frases e dos seus segmentos.
• Foram bastante reduzidos os avanços, atrasos, saltos irregulares e diferenças entre motores de voz.
• A posição correta é melhor preservada depois de pausar, retomar, pesquisar no documento ou mudar de motor.

Gravação de podcast em ficheiros separados
• Adicionada a opção “Guardar o microfone e o áudio do sistema ou das aplicações em ficheiros separados”.
• Ao gravar simultaneamente o microfone e outra fonte, o Sonarpad pode criar um ficheiro apenas com o microfone e outro com o áudio do sistema, de uma aplicação ou das aplicações selecionadas.
• A separação está disponível em MP3 e WAV.
• Se a opção estiver desativada, continua a ser criado um único ficheiro misturado.
• Os ficheiros separados facilitam o ajuste de volume, a remoção de ruído e a edição posterior de podcasts, entrevistas e tutoriais.

Gravações de rádio programadas
• As gravações de rádio podem agora ser programadas antecipadamente.
• É possível escolher a estação, o dia, a hora e os minutos de início e a duração.
• Está disponível uma duração personalizada de 1 a 1.440 minutos.
• A gravação pode ser executada uma vez, diariamente ou semanalmente.
• A janela mostra mais claramente as gravações ativas e programadas, a data e hora previstas, a duração e o tempo restante.
• O Agendador de Tarefas do Windows pode iniciar automaticamente a gravação mesmo quando o Sonarpad não está aberto.

Calendário
• Adicionado um calendário completo e acessível por teclado.
• Permite consultar dias anteriores e seguintes, regressar rapidamente a hoje e conhecer feriados e efemérides.
• Adicionados o santo e a citação do dia, que podem ser lidos, ouvidos ou copiados.
• Os lembretes podem ser criados, alterados, eliminados, adiados e marcados como concluídos.
• Os avisos podem surgir à hora exata ou antecipadamente e utilizar o agendamento do Windows mesmo com o Sonarpad fechado.

Meteorologia
• Adicionada uma secção de previsão meteorológica.
• É possível procurar uma cidade e voltar rapidamente aos locais consultados recentemente.
• Estão disponíveis as condições atuais, temperatura, valores mínimo e máximo, humidade, probabilidade de precipitação e previsão para os dias seguintes.
• É possível escolher Celsius, Fahrenheit ou seleção automática.

Filmes no cinema
• Adicionada uma secção para filmes atualmente em exibição e próximas estreias.
• Estão disponíveis pesquisa por título, sinopse, data de estreia e reprodução do trailer.

Síntese de voz Google
• Integrado o Google TTS para leitura de documentos e criação de audiolivros.
• Adicionado um gestor para mostrar as vozes, filtrá-las por idioma, descarregá-las e remover as que já não são necessárias.
• É possível ajustar velocidade, volume e tom.
• O tom das vozes Google Natural é aplicado diretamente pelo motor para um resultado mais natural e estável.
• Melhoradas a rapidez e a fiabilidade do Google TTS, adaptando os limites de síntese à velocidade escolhida.
• Reduzidos os tempos de espera desnecessários e melhorada a gestão de erros e interrupções.

Índice de documentos EPUB
• O Sonarpad reconhece agora o índice incorporado nos livros EPUB.
• A sua presença é anunciada e pode ser aberto no menu Ver.
• Os capítulos e subcapítulos são apresentados hierarquicamente.
• Premir Enter leva imediatamente ao local selecionado.

Notícias e fontes RSS
• A secção Notícias foi ampliada com novas ferramentas de pesquisa e organização.
• Adicionada a escolha do idioma das notícias.
• É possível pesquisar nas fontes RSS e consultar notícias da própria cidade.
• As fontes da comunidade podem ser exploradas, adicionadas à coleção pessoal e enviadas à comunidade Sonarpad.

Gravação de podcast
• É possível gravar apenas o microfone, todo o áudio do sistema, uma aplicação, várias aplicações selecionadas ou o microfone e as aplicações ao mesmo tempo.
• Podem ser escolhidos o dispositivo e a fonte, os volumes ajustados separadamente e os níveis acompanhados em tempo real.
• Adicionadas pausa e retoma, saída MP3 ou WAV, seleção do bitrate MP3 e da pasta de destino.
• O computador pode ser mantido ativo durante a gravação.

Rádio
• A secção Rádio foi profundamente reorganizada.
• As estações podem ser pesquisadas por nome ou texto livre, idioma, país, cidade, género musical ou categoria.
• Melhorada a gestão de favoritos e adicionado o restabelecimento rápido de todos os filtros.
• As estações podem ser enviadas à comunidade Sonarpad.
• Adicionadas a gravação em direto, a modalidade “Gravar e reproduzir”, a lista de gravações e a respetiva gestão e eliminação.
• As gravações de rádio são guardadas numa pasta própria dentro do diretório geral de gravações.

Reprodução multimédia
• A estabilidade do leitor multimédia foi significativamente melhorada.
• Corrigido um problema que podia bloquear o mpv e melhorada a comunicação com o leitor.
• Melhorada a abertura de diferentes tipos de ficheiros multimédia.
• O Sonarpad memoriza agora o volume utilizado durante a reprodução.
• Melhorada a gestão de transmissões e gravações.
• Corrigida a abertura de ficheiros a partir do Windows por duplo clique ou “Abrir com”.

Documentos PDF
• Adicionado o reconhecimento de campos de formulário em PDF.
• O Sonarpad pode encontrar campos preenchíveis, apresentá-los numa forma textual acessível, permitir a sua edição e guardar os dados no PDF.
• Corrigido o cálculo da posição do cursor durante a leitura, sobretudo com caracteres multibyte e estruturas complexas.

Acessibilidade e teclado
• Melhorado o funcionamento dos comandos normais de edição em todo o programa.
• Copiar, cortar, colar, selecionar tudo, anular e refazer são corretamente enviados para o campo com foco, incluindo janelas secundárias e caixas de diálogo.
• Corrigido um problema de atualização das linhas Braille.
• Melhorada a gestão do foco e corrigida a escolha do idioma na Wikipédia.
• Adicionada a possibilidade de agrupar por categoria as funções do menu Ferramentas.
• Adicionadas ações configuráveis para abrir rapidamente Calendário, Meteorologia e Filmes no cinema.

Audiolivros
• Melhorada a criação de audiolivros quando existem caixas de diálogo ou janelas modais abertas.
• A gestão do progresso é mais robusta e ignora atualizações de áudio antigas.
• O Google TTS também pode ser utilizado para criar audiolivros com controlo de velocidade, volume e tom.

Inteligência artificial
• O modelo Gemini predefinido foi atualizado para `gemini-3.5-flash`.

Correções gerais
• Corrigidos vários bloqueios durante a reprodução com mpv.
• Corrigida a abertura de alguns ficheiros de áudio e vídeo.
• Melhorada a gestão dos comandos enviados para o leitor.
• Corrigida a reposição do cursor durante a leitura.
• Melhorada a estabilidade da criação de audiolivros.
• Melhorada a gestão geral de multimédia, RSS, rádio e EPUB.

Versão 0.7.1 – 2026-05-13

Novidades e melhorias
• Criado o site oficial sonarpad.com, um novo ponto de referência para acompanhar as últimas novidades, descarregar a versão mais recente do programa, ler os comentários dos visitantes e, no futuro, ouvir também todos os podcasts do Sonarpad. No menu Ajuda também foi adicionada a opção “Visitar sonarpad.com”, para abrir rapidamente o site oficial.
• Corrigido o problema em que ficheiros com acentos ou caracteres especiais causavam erro ao iniciar a transcrição de voz.
• A partir de agora, no menu Ver, opções como Quebra automática de linha e Mostrar vídeo durante a reprodução irão aparecer sempre com o estado correto, ativadas ou desativadas.
• Melhorada a pesquisa no YouTube, permitindo voltar com Esc à página ou ao ecrã anterior.
• Adicionada uma verificação preliminar para confirmar se um vídeo pode ser reproduzido. A reprodução também foi melhorada: o Sonarpad agora consegue reproduzir vídeos ou playlists marcados como mix, que antes não eram reproduzidos.
• Melhorada a gestão dos marcadores automáticos. Antes, se a opção Marcadores automáticos estivesse ativa e fosse depois desativada, esses marcadores permaneciam; agora o programa ignora-os corretamente até que a opção seja reativada. Além disso, ao chegar ao fim de um ficheiro multimédia, o marcador é apagado automaticamente.
• Melhorada a gestão das etiquetas com os diálogos ativos. O Sonarpad agora gere corretamente ambas as funções, permitindo inserir etiquetas mesmo quando a opção de diálogos está ativa.
• Melhoradas as definições de voz, separando claramente cada motor para tornar os ajustes mais precisos. Os perfis de voz conservam corretamente as definições de cada motor individual: Edge, Sapi5 e Sapi4.
• Adicionada uma etiqueta para inserir pausas, diretamente a partir das opções ou do painel de vozes premindo Tab a partir do editor. As opções são: 250 ms, 500 ms, 1 segundo, 2 segundos ou duração personalizada.
• Corrigido o comportamento ao reproduzir um vídeo do YouTube e iniciar a transcrição. Agora, ao voltar com Alt+Tab, o foco ficará corretamente no botão Cancelar da transcrição em curso.
• As transcrições passam agora a ser guardadas automaticamente no fim do processo.
• Melhorada a importação da Wikipédia. É possível escolher ler apenas uma secção e depois, a partir do artigo, premir Esc para voltar à pesquisa, ou importar o artigo completo. Também é possível escolher o idioma da Wikipédia a consultar.
• Adicionada uma secção de rádios de todo o mundo, onde será possível procurar uma rádio por país, idioma e género. Também será possível adicionar rádios locais à base de dados do Sonarpad, para que outros utilizadores possam ouvi-las. Também é possível adicionar uma rádio aos favoritos.
• Adicionada uma secção de percursos para calcular rotas escolhendo o meio: a pé, de bicicleta, de carro ou em cadeira de rodas. É possível escolher se o percurso deve ser mais curto ou mais rápido e se devem ser mostrados os municípios atravessados. Depois de importar o percurso, também será possível guardar o mapa visual no menu Ficheiro, Guardar imagem.
• Adicionada a opção Imprimir ao menu Ficheiro. O Sonarpad imprimirá ficheiros TXT usando o próprio programa e usará o programa associado para outros ficheiros, como DOCX, PDF e semelhantes, para preservar ao máximo o layout original.
• Integrado no Sonarpad um serviço de tradução para cada documento, acessível a partir do menu contextual do editor. O utilizador poderá usar gratuitamente DeepL e Google Translate sem inserir nenhuma chave API; inserindo uma chave API Gemini, poderá traduzir usando Gemini.
• No menu de tradução, o utilizador poderá escolher o idioma de destino. O menu reorganiza-se automaticamente: se um utilizador escolher primeiro inglês, depois francês e depois italiano, estas três opções ficarão no topo do menu de idiomas.
• Se o utilizador inserir a sua chave API Gemini, poderá também aceder à função Resumir texto, sempre disponível no menu contextual, para resumir qualquer artigo.
• Adicionado ao menu Reproduzir, visível durante a reprodução de um ficheiro multimédia, um menu para dividir o média atual. Funciona com MP3, MP4 e outros formatos, dividindo por número de partes ou pela duração de cada parte.

Versão 0.7.0 – 2026-04-25

Novidades
• Adicionado suporte ao player mpv para reprodução em streaming. Vídeos do YouTube e de sites compatíveis agora são reproduzidos instantaneamente; se o usuário optar por salvá-los, eles são baixados como antes. Ao transcrever conteúdo em streaming, ele é primeiro baixado e depois transcrito. O player mpv também é utilizado para abrir vídeos locais e gerenciar legendas, garantindo maior compatibilidade com vários formatos que antes não eram bem suportados.
• Melhorada a gravação de podcasts do áudio do sistema: agora é possível escolher entre gravar todo o áudio do sistema, uma única aplicação ou várias aplicações ao mesmo tempo. Esta opção está integrada com a gravação normal, pelo que continua a ser possível ativar ou desativar o microfone separadamente.
• Adicionado o idioma hindi. Interface traduzida e adicionados RSS, changelog e guia do Sonarpad.
• Adicionada uma opção na aba Editor para mover sempre o cursor para o início da linha ao usar as setas para cima e para baixo.
• Adicionada uma opção no menu "Converter áudio" para converter áudio em M4B.

Correções
• Nos comentários do YouTube abertos a partir de "Reproduzir áudio por streaming...", o Sonarpad passa agora a carregar inicialmente apenas os primeiros 50 comentários principais, incluindo sempre todas as respostas desses comentários, e adiciona no fim uma opção para carregar todos os comentários a pedido.
• Os marcadores passam agora a ser mostrados e geridos por ordem de posição, tanto em documentos de texto como em ficheiros multimédia, em vez de seguirem a ordem de criação. Se já existir um marcador na mesma posição, ele deixa de ser adicionado novamente.
• Foi adicionada uma opção no menu Marcadores que, quando ativada, permite a gestão automática dos marcadores. Ao reproduzir um ficheiro local ou em streaming e fechá-lo, o Sonarpad define automaticamente um marcador com base na posição alcançada e, ao reabrir o ficheiro, retoma a partir desse ponto. O mesmo acontece com os ficheiros de texto: se um texto for aberto e o cursor for movido, o Sonarpad irá recordar essa posição ao fechar; se a leitura for iniciada, será guardada a última frase lida e a leitura continuará exatamente a partir daí.
• Foi adicionada ao menu Ver uma opção para mostrar a renderização de vídeo para ficheiros locais ou em streaming. O conteúdo de vídeo é apresentado numa janela ampliada, onde todos os comandos ficam ocultos, exceto quando se prime a tecla Alt ou se move o rato para a parte superior da janela. Desta forma, os utilizadores com baixa visão deverão ter um conteúdo maior e mais fácil de utilizar.

Versão 0.6.9 – 2026-04-08

Correções
• A experiência de Localizar nos ficheiros foi melhorada: ao abrir Procurar pasta, o foco vai diretamente para a lista de pastas; ao abrir um resultado com Enter, todos os comandos de teclado continuam a funcionar; ao premir Esc, volta ao resultado anteriormente selecionado; e ao regressar com Alt+Tab, o foco vai para o campo de pesquisa ou para a lista de resultados, se esta estiver aberta.
• O F5 iniciava sempre a leitura desde o início. Isso foi corrigido e a leitura passa agora a começar na posição atual do cursor, preservando também `Shift+F5` e `Ctrl+F5` para ir para a frase anterior ou seguinte.
• Depois de usar Ir para a linha, ao premir Esc o foco podia sair do Sonarpad. Agora volta corretamente ao editor.
• A opção `Quebra automática de linha` agora é aplicada imediatamente também aos documentos já abertos, sem precisar reabrir o ficheiro.

Versão 0.6.8 – 2026-04-07

Novidades
• Adicionado um novo item no menu Reproduzir que permite transcrever qualquer ficheiro de áudio ou vídeo com o Whisper. Nas Opções existe uma nova secção chamada «IA e Transcrição», onde é possível escolher o modelo, ativar o suporte opcional a CUDA para placas gráficas NVIDIA, manter o idioma original e ativar ou desativar as marcas temporais.
• Foi adicionada ao menu Reproduzir a nova ação `Transcrever pasta atual`, que transcreve todos os ficheiros de áudio suportados da pasta do media aberto e junta tudo num único documento, com janela de progresso dedicada, indicação do ficheiro atual e possibilidade de cancelar. Também pode ser iniciada com `Alt+Shift+C`.
• Adicionada a possibilidade de usar ditado por voz offline, com o mesmo funcionamento da transcrição de áudio. Por predefinição, prima `Ctrl+Shift+Espaço` para iniciar o ditado e prima o mesmo atalho novamente para o terminar; o atalho pode ser personalizado nas Opções. A partir da segunda ativação, o ditado fica mais rápido porque o motor permanece pronto na memória; em PCs com menos de 4 GB de RAM, este pré-carregamento e reutilização são desativados automaticamente.
• Foi adicionada nas Opções do editor uma nova definição, desativada por predefinição, que faz com que `Esc` feche a janela do editor.
• A pesquisa de podcasts passa agora a usar `iTunes + Spreaker` por predefinição, com filtragem de resultados duplicados quando o mesmo podcast está presente em ambas as plataformas.
• Melhorada a pesquisa e navegação de podcasts Apple: a pesquisa de podcasts, a navegação por categorias e os top podcasts por categoria passam agora a usar o país selecionado para o diretório de podcasts. Em Opções > RSS / Podcast, pode deixar em `Automático` para usar o país do sistema ou escolher manualmente outro país.
• O limite de resultados para as categorias de podcasts Apple foi aumentado. Na primeira abertura continuam a ser carregados apenas os primeiros 50 resultados como antes; ao escolher `Carregar mais resultados`, o Sonarpad carrega até 200 resultados no total (limite imposto pela Apple) e permite navegar pelas páginas seguintes mantendo uma experiência fluida.
• O Sonarpad está agora disponível também no Mac, embora com um conjunto de funcionalidades parcial. Ligação do projeto: https://github.com/Ambro86/Sonarpad-Mac

Melhorias
• Foram adicionados mais de 50 países selecionáveis para o diretório de podcasts, permitindo escolher entre muitos mais catálogos nacionais.
• "Reproduzir áudio por streaming..." agora também permite pesquisar no YouTube escrevendo qualquer texto ou colar a ligação de um canal ou de uma playlist do YouTube para mostrar os respetivos resultados.
• A apresentação dos resultados em "Reproduzir áudio por streaming..." foi melhorada: as entradas do YouTube agora incluem título, duração, canal e visualizações num formato mais claro.
• "Reproduzir áudio por streaming..." passa agora a suportar também os comentários do YouTube: podem ser abertos a partir do menu contextual, é possível ler as respostas e expandir os tópicos de comentários com a Seta para a direita.
• Foram adicionados favoritos do YouTube para canais e playlists em "Reproduzir áudio por streaming...": podem ser adicionados a partir dos resultados através do menu contextual, abertos diretamente a partir da lista Favoritos acessível com Tab logo após o campo de URL/pesquisa do YouTube e removidos mais tarde dessa mesma lista também pelo menu contextual. Nos resultados de pesquisa do YouTube, o menu contextual está disponível apenas para canais e playlists.
• "Reproduzir áudio por streaming..." agora pode pedir credenciais quando um site exige início de sessão. O utilizador pode inseri-las, guardá-las para esse site e gerir depois as credenciais guardadas em Opções > Áudio.
• Melhorado o foco durante "Reproduzir áudio por streaming...", para que a janela de progresso permaneça mais estável durante a descarga e a conversão.
• Adicionadas duas novas ações de leitura no menu Voz: `Frase anterior` e `Próxima frase`, com atalhos configuráveis para saltar durante a leitura do texto.
• O atalho predefinido de `Executar ficheiro com interpretador` é agora `Ctrl+Shift+F5`, para que `Shift+F5` possa ser usado por predefinição para `Frase anterior`.
• Adicionada a gestão de perfis de voz em Opções > Voz: é possível adicionar, renomear e eliminar perfis.
• Alargadas em Opções > Áudio as opções do intervalo de retrocesso durante a reprodução, com novos valores de 1 segundo até 2 horas.
• Adicionada a tradução russa graças a Dmitriy.
• Adicionada em Opções > Áudio uma nova opção para escolher o formato do nome das partes do audiolivro: `Título + número`, `Somente número` ou `Número + título`.
• Adicionada no menu de contexto dos artigos RSS a ação para adicionar o artigo aos favoritos.
• A fonte RSS "Favoritos" pode ser eliminada e é recriada automaticamente quando um novo artigo é adicionado aos favoritos.
• Adicionados atalhos de teclado RSS para mover as fontes para cima/para baixo: `Ctrl+Shift+Seta para cima` e `Ctrl+Shift+Seta para baixo`.
• Melhorada a janela RSS com uma pré-visualização integrada do artigo, permitindo consultar o texto diretamente ali e alcançá-lo rapidamente com Tab antes de abrir o artigo completo no editor.
• Adicionada no RSS uma entrada explícita «Carregar mais notícias» no fim das fontes quando existem mais itens disponíveis; ao premir Enter é carregado o bloco seguinte e o foco passa para o primeiro artigo novo.
• No dicionário de voz, ao adicionar ou editar uma substituição, existe agora uma caixa «Distinguir maiúsculas e minúsculas» para decidir se cada substituição deve respeitar ou ignorar a capitalização.
Correções
• "Reproduzir áudio por streaming..." passa agora a respeitar o limite de cache de podcasts já definido nas Opções, e esse mesmo limite também se aplica à reprodução de audiodescrições.
• Corrigida a importação a partir da Wikipédia, que em algumas páginas não importava corretamente as citações presentes no texto.
• Melhorado o parser de páginas web: em algumas páginas WordPress não eram incluídos os itens de listas nem alguns títulos de secção.
• Agora, ao usar «Ir para a linha», o campo é pré-preenchido com a linha atual.
• Corrigida a exportação OPML de podcasts e feeds RSS, que agora gera ficheiros aceites pelo iTunes.
• Corrigida a transcrição de ficheiros multimédia: agora, ao fechar com Alt+F4 o documento gerado, o Sonarpad pergunta se pretende guardar o ficheiro e propõe o nome correto com base no nome do ficheiro transcrito, em vez da primeira linha do texto.
• Adicionadas mensagens de confirmação localizadas para a correta importação e exportação OPML de fontes RSS e podcasts.
• Foi corrigido um problema em que, em "Reproduzir áudio por streaming...", ao digitar um texto de pesquisa e selecionar um canal do YouTube nos resultados, o programa podia parecer bloqueado em vez de abrir os vídeos desse canal.
• Corrigido um erro em que a lista de ficheiros abertos era mostrada no menu Ajuda em vez do menu Janela.
• Corrigido um caso limite no streaming em que a reprodução podia iniciar, mas a janela “Transferência de streaming” permanecia aberta quando o ficheiro descarregado já correspondia ao formato de destino.
• Corrigido o comportamento de conversão no streaming MP3: quando o stream já está em MP3 e o utilizador escolhe um bitrate MP3 explícito (por exemplo 128 kbps), o Sonarpad agora recodifica para o bitrate selecionado em vez de saltar a conversão.
• Corrigido o atalho `Alt+Shift+L`: agora abre corretamente a lista de capítulos durante a reprodução.
• Corrigido o atalho `Alt+Shift+T`: agora inicia corretamente «Transcrever áudio atual» em vez de abrir o menu Ferramentas.
• Se já estiver a ser reproduzido um áudio, ao iniciar a transcrição o Sonarpad coloca esse áudio automaticamente em pausa antes de começar.
• Corrigido um problema em que, ao importar um artigo da Wikipédia, a importação podia ter êxito mas o texto do artigo não era mostrado no ecrã.
• Adicionado suporte a capítulos de podcast incorporados em ficheiros multimédia locais (por exemplo, metadados de capítulos MP3): quando o feed/URL não fornece capítulos, o Sonarpad passa a carregá-los do ficheiro descarregado em segundo plano, permitindo início imediato da reprodução e aplicação dos capítulos assim que ficam disponíveis.
• Corrigido o carregamento de capítulos para episódios de podcast descarregados e abertos como ficheiros multimédia locais normais: os capítulos incorporados passam agora a estar disponíveis também nesse caso, e não apenas quando a reprodução começa a partir da janela Podcasts.
• Corrigida a finalização dos audiolivros MP3 com SAPI4 e SAPI5: o ficheiro final passa agora a ser finalizado corretamente, evitando ficheiros incompletos ou frágeis após exportações longas.
• Adicionada uma barra de progresso explícita para a fase de finalização em todos os modos de criação de audiolivros: após a criação, o Sonarpad anuncia e mostra a finalização com progresso visível.
• Corrigido um erro nas vozes de diálogo: os parâmetros de velocidade/tom/volume da primeira e da segunda voz de diálogo são agora aplicados corretamente durante a síntese.
• Melhorada a deteção de codificação para ficheiros japoneses `.txt`: adicionado fallback seguro Shift_JIS/CP932 em casos de mojibake, preservando o comportamento existente para UTF/diacríticos/chinês.
• Refatoração interna de segurança: conversão para implementações safe sempre que possível e redução drástica das linhas de código unsafe.

Versão 0.6.7 – 2026-03-02
Melhorias
• Ora il programma riesce a gestire Sostituisci tutto in modo massivo su file grandi con un gran numero di sostituzioni.
• Tradução polaca atualizada graças ao DJ Graco.
• Adicionada a tradução lituana.
• Adicionada a tradução chinesa.
• A partir de agora, compilações beta frequentes serão publicadas na seção Releases do projeto, para que os usuários possam testar as novas alterações antes da próxima versão estável.
• Adicionado o atalho `Ctrl+.` para inserir o caractere de reticências (…).
• Melhorado o suporte a capítulos de podcast: a navegação por capítulos está agora mais fiável, incluindo episódios diretos/streaming em que os capítulos não estão incorporados no ficheiro MP3, usando metadados de capítulos do feed/URL como fallback quando disponíveis. Adicionados os atalhos `Ctrl+Alt+PageUp` (capítulo anterior) e `Ctrl+Alt+PageDown` (capítulo seguinte).
• Reorganizadas as pastas de saída em `Documentos\\Sonarpad`: os ficheiros agora são guardados nas subpastas dedicadas `audiobooks`, `documents`, `recordings` e `media`, com migração automática dos caminhos antigos.
• Melhorado o suporte para ficheiros de texto muito grandes (incluindo 60 MB): abertura e navegação linha a linha mais fluídas, especialmente com leitores de ecrã.
• Guias atualizados para todos os idiomas e recursos de localização atualizados em toda a app, incluindo textos de doações e traduções do instalador NSIS (novas strings em chinês simplificado e lituano, além da conclusão da tradução ucraniana do setup).
• Adicionado suporte global de proxy de rede (HTTP/HTTPS e SOCKS5/SOCKS5H) para funcionalidades online, com validação ao guardar Opções: proxies inválidos são avisados e removidos automaticamente.
• Adicionada uma nova função em Ferramentas: "Reproduzir áudio por streaming...", que permite colar um URL (YouTube ou link multimédia direto), escolher o formato de saída e o perfil de qualidade/bitrate (incluindo qualidade/bitrate original para MP3 e MP4) e iniciar a reprodução no leitor de áudio do Sonarpad.
• Adicionado suporte à tecla multimédia de sistema Reproduzir/Pausar (auscultadores/teclado): agora controla tanto a reprodução multimédia como a pausa/retoma da leitura de texto (com prioridade para o leitor multimédia quando ambos estão ativos).
• Adicionada uma nova opção em Ficheiro > Ficheiros recentes: "Limpar ficheiros recentes" para esvaziar rapidamente a lista de documentos recentes.
• Ampliadas as opções de bitrate na conversão de áudio e na gravação de podcast: adicionados valores mais baixos (64/96 kbps) e MP3 estendido até 320 kbps, com validação e tratamento do encoder alinhados.
• Ampliadas as opções de divisão de audiolivros por tempo até 60 minutos.
• Melhorada a divisão de audiolivros por partes: agora o número de partes pode ser inserido manualmente, com validação de 1 a 100.
• Adicionado o novo modo Ver > Somente leitura para bloquear edições acidentais no texto, mantendo leitura e navegação completas dos documentos.
• Adicionada uma barra de progresso acessível durante as atualizações do programa, para que leitores de ecrã possam acompanhar em tempo real o progresso da transferência.
• Adicionada uma nova barra de estado discreta na janela principal com contagem de caracteres, palavras e posição linha/coluna (por exemplo: "Caracteres (com espaços): 11. | Palavras: 2. | Ln 1, Col 12"), sem interferir com o foco do NVDA.
• Adicionada uma nova opção no menu Ver para quebra automática de linha, permitindo ativar/desativar rapidamente sem abrir as Opções.
• Adicionadas em Editar > Texto novas ações para aumentar/reduzir recuo, com atalhos Ctrl+Shift+. (indentar) e Ctrl+Shift+, (desindentar), porque quando “Mostrar vozes no editor” está ativo a tecla Tab fica reservada para a navegação do painel de vozes.
• Adicionada a exibição localizada de data e hora em artigos RSS e episódios de podcast, com formato adaptado ao idioma da interface.
• Adicionada no menu de contexto RSS uma nova ação para partilhar por e-mail o artigo selecionado.
• Adicionadas opções granulares de confirmação de remoção em Opções > RSS e podcast: para RSS (feed/artigo/ambos/nenhum) e para Podcasts (podcast/episódio/ambos/nenhum).
• Adicionada cópia rápida de RSS configurável com Ctrl+C (Opções > RSS e podcast): copiar título, URL, conteúdo do artigo ou tudo junto.
• Fluxo de RSS unificado: “Adicionar fonte” agora aceita tanto URL de feed quanto palavras-chave (com geração automática do feed do Google News), sem necessidade de pesquisa separada.
• Ao premir Ctrl+A, o programa agora anuncia a conclusão da ação para um feedback mais claro em leitores de ecrã.
• Adicionado Shift+F3 para "Localizar anterior" no menu Editar, em complemento ao F3 "Localizar seguinte".
• Melhorada a mensagem de substituição com singular/plural corretos (por exemplo, “1 substituição realizada” vs “2 substituições realizadas”).
• Adicionada na janela do dicionário a seleção de idioma de pesquisa, com Auto (idioma da interface) por padrão e possibilidade de escolha manual.
• Adicionada uma nova aba de Atalhos nas Opções para personalizar combinações de teclas, com deteção de conflitos e aviso quando um atalho já está atribuído a outra ação.
• Adicionado suporte inicial a parâmetros de linha de comandos: `-h`/`--help` mostram a ajuda rápida e `--version` mostra a versão do programa.
• Melhorada a clareza do ajuste manual de velocidade e tom: os campos manuais agora usam uma escala centrada em 100, onde 100 corresponde ao valor normal.
• Melhorada a seleção de vozes Microsoft em Opções > Voz e no painel de vozes do editor: foi adicionada uma combobox de idioma localizada para filtrar vozes por idioma, mantendo o modo “apenas vozes multilíngues” como lista única sem divisão por idioma (com a combobox de idioma oculta quando ativa).
• Adicionada a configuração de voz para diálogos em Opções > Voz com navegação completa por Tab, usando o mesmo modelo de vozes da interface principal (motor, filtro de idioma Edge, voz e velocidade/tom/volume com etiquetas); adicionada também uma segunda voz de diálogos opcional com os mesmos controles (motor, filtro de idioma Edge, voz, velocidade/tom/volume) para alternar diálogos; as regras de diálogos são guardadas na configuração `.ini`, sem modificar o texto do documento.
• Melhorada a etiqueta de Desfazer: a opção Editar > Desfazer agora mostra qual ação será desfeita (por exemplo, edição de texto, comentar/descomentar linhas ou inserção de tag de voz), mantendo-se indisponível quando não há nada para desfazer.
Correções de bugs
• Corrigido o suporte de abertura RTF: os ficheiros `.rtf` agora são extraídos e mostrados como texto legível, em vez de markup RTF bruto (ex.: `{\\rtf1...}`).
• Corrigida a abertura de ficheiros de texto chineses em codificação GB18030/GBK: o Sonarpad agora deteta e descodifica corretamente, evitando texto ilegível (mojibake).
• Melhorada a criação de audiolivros M4B com metadados e marcadores de capítulos; corrigido o problema "chipmunk" (voz demasiado aguda/rápida) nos ficheiros M4B gerados.
• Corrigida a interface de bitrate na janela de gravação de audiolivro: removidos textos hardcoded em italiano e adicionada a opção 64 kbps entre os bitrates selecionáveis.
• Corrigido "Guardar tudo" (Ctrl+Shift+S): agora todos os documentos abertos modificados são detetados de forma fiável (incluindo separadores novos/sem guardar) e o Guardar tudo grava cada um corretamente, abrindo "Guardar como" quando necessário.
• Corrigida a ordenação dos artigos RSS do Google News: quando a data está disponível, os artigos agora são mostrados do mais recente para o mais antigo.
• Corrigida a associação de etiquetas no NVDA na janela do dicionário: o campo de pesquisa e a lista de idioma agora anunciam a etiqueta correta.
• Corrigida a navegação por teclado na janela Propriedades de RSS/Podcast: Tab/Shift+Tab agora alcançam o botão OK, Enter ativa o OK, Esc fecha com segurança e o foco volta corretamente à lista RSS/Podcast.
• Corrigido o histórico de desfazer em RSS/Podcast: o Ctrl+Z agora suporta desfazer em múltiplos níveis para remoções (artigos/episódios e fontes), e não apenas a última ação.
• Melhorados os anúncios de remoção em RSS/Podcast com mensagens explícitas (RSS removido, artigo RSS removido, episódio de podcast removido).
• Melhorado o comportamento de foco após remover/desfazer em RSS/Podcast: no RSS, o primeiro feed volta a ser selecionado de forma fiável quando necessário, e foram reduzidas repetições de anúncios do leitor de ecrã durante a re-seleção atrasada.

Versão 0.6.6 – 2026-02-13
Melhorias
• Adicionada "Formatação automática para TTS" no menu Editar para preparar rapidamente o texto para voz (remove markdown/aspas e recompõe linhas quebradas).
• Melhorada a inserção de tags de voz: quando há texto selecionado, as tags passam a ser aplicadas corretamente tanto em uma única linha quanto em seleção multilinha.
• Adicionada uma opção nas Configurações de áudio para escolher a pasta padrão de gravação de audiolivros (padrão: Documentos\\Sonarpad Audiobooks).
• Na janela de gravação de audiolivro, quando a divisão em partes está ativa, foi adicionada uma nova opção (ativada por padrão) para criar uma subpasta dedicada às partes geradas.
• A exportação de audiolivros agora guarda MP3 em estéreo com bitrate escolhido pelo utilizador para vozes Edge, SAPI5 e SAPI4.
• Adicionado suporte a vozes SAPI5 de 32 bits via bridge, para usar também vozes disponíveis apenas em motores de 32 bits.
• Reorganizadas as funcionalidades de voz num menu dedicado "Voz e áudio" e adicionada/esclarecida a opção "Converter áudio", útil para converter qualquer ficheiro multimédia suportado para MP3, AAC, OGG, Opus, FLAC, WAV e AIFF.
• Adicionada a remoção de artigos RSS individuais e episódios de podcast individuais (tecla Delete + menu de contexto com confirmação), sem remover toda a fonte RSS/podcast, com anulação da última remoção (artigo/episódio individual ou fonte RSS/podcast completa).
• Adicionada a exportação de feeds RSS para OPML na janela RSS, para guardar e reimportar facilmente as fontes atuais.
• Adicionada a função "Pesquisar RSS por palavra-chave" na janela RSS: ao inserir uma palavra-chave, o Sonarpad gera automaticamente o URL RSS do Google News e abre a janela de adicionar fonte já pré-preenchida, permitindo criar um feed temático num único passo.
• Adicionada a tradução sérvia graças a Mila Kuran.
• Adicionada a tradução ucraniana graças a Ivan Shtefuriak.
• Adicionada a abertura múltipla de ficheiros multimédia: ao abrir vários ficheiros de uma vez é criada uma fila de reprodução em vez de substituir o ficheiro atual.
• Adicionados atalhos de avanço/retrocesso variável durante a reprodução: com base de 1 minuto, Esquerda/Direita avança 60s, Shift+Esquerda/Direita avança 20s e Ctrl+Esquerda/Direita avança 3 minutos.
• Adicionados atalhos de faixa anterior/seguinte no leitor: Ctrl+PageUp e Ctrl+PageDown.
• Adicionada a opção "Repor volume" e agrupadas as ações de reposição num submenu dedicado "Repor" em Reprodução, juntamente com "Repor velocidade" e "Repor tom".
• Melhorias no instalador: o setup.exe agora permite escolher entre associar todos os tipos de ficheiro suportados ou selecionar manualmente as extensões; o MSI também passa a oferecer seleção por extensão na árvore de funcionalidades (o padrão mantém-se: tudo ativo).
• Adicionado o novo menu "Janela" com a opção "Documentos abertos..." para alternar rapidamente para qualquer ficheiro atualmente aberto.
• Atualizada a opção Ver > Fonte: o seletor completo foi substituído por um submenu rápido com fontes comuns (Arial, Calibri, Consolas, Segoe UI, Tahoma, Verdana, Times New Roman, Georgia), mantendo o tamanho de texto atual.
• Melhorada a leitura de RSS e podcasts com dois avisos distintos: os nós da fonte anunciam "novos itens" quando um feed/podcast tem novidades, enquanto artigos RSS e episódios de podcast individuais anunciam "não lido"/"não reproduzido"; este comportamento pode ser desativado nas Opções.
Correções de bugs
• Corrigida a extração de texto EPUB para livros com comentários HTML inline (<!-- ... -->): o texto dos capítulos agora é analisado corretamente em vez de ser parcialmente ou totalmente ignorado.
• Corrigido o dicionário Wiktionary em espanhol e o cache do dicionário: palavras como "agua" agora são encontradas corretamente e entradas antigas de "Palavra não encontrada" não são mais reutilizadas.
• Corrigida a codificação na importação de artigos RSS para algumas fontes em espanhol (ex.: El Mundo): acentos e "ñ" agora são preservados corretamente no editor temporário.
• Corrigida a descodificação ANSI de ficheiros da Europa Central (ex.: checo/polaco): o Sonarpad agora distingue melhor UTF-8 e ANSI e escolhe a code page correta (incluindo Windows-1250), evitando diacríticos corrompidos.
• Corrigida a persistência de fontes RSS com parâmetros na URL (ex.: `rss.aspx?c=...`): esses feeds agora são guardados e restaurados corretamente após reiniciar o Sonarpad.
• Corrigida a abertura de ficheiros ponteiro do Google Drive (`.gdoc`, `.gsheet`, `.gslides`) a partir do menu de contexto do Explorador: quando a leitura direta falha com “Incorrect function (os error 1)”, o Sonarpad agora usa fallback por shell-open e o documento abre corretamente.
• Corrigida a leitura de ficheiros Excel legacy `.xls` (Excel 2010): ficheiros binários antigos são agora detetados/descodificados corretamente em vez de mostrar texto corrompido (ex.: `ÐÏ_à¡±...`).
• Corrigido o fluxo de anúncio do corretor ortográfico: os erros voltam a ser anunciados ao rever o texto mais tarde, e o mesmo erro é novamente reportado se for apagado e reescrito.
• Corrigidas as operações de texto por linha (ex.: Ctrl+Q / Ctrl+Shift+Q, ordenar/inverter/linhas únicas/unir linhas): ao selecionar apenas uma linha com Shift+Seta para baixo, as linhas adjacentes não são mais unidas nem truncadas.
• Corrigido o comportamento em seleções multilinha nas operações por linha (Ctrl+Q / Ctrl+Shift+Q e ferramentas relacionadas): quando o RichEdit devolve separadores de linha apenas com CR, agora são normalizados corretamente e todas as linhas selecionadas são processadas sem cortar o primeiro carácter.
• Ampliada a normalização de entrada TTS para símbolos visíveis de espaço/tab/nova linha (␠/U+2420, ␣/U+2423, ␉/U+2409, ␊/U+240A, ␍/U+240D, ␤/U+2424), que com vozes multilíngues podiam causar repetição de parágrafos.
• Refinada a sanitização do texto para Edge TTS com uma única pipeline de validação: normalização de espaços estranhos/invisíveis, compactação de sequências longas de pontuação (como "...", "!!!", "???") e descarte de trechos compostos só por pontuação para evitar loops de reprodução.
• Corrigido o anúncio do tempo de reprodução (Ctrl+I) para streams MP3/podcast: o tempo atual agora é limitado à duração da faixa, e a reprodução é interrompida automaticamente se a posição ultrapassar o fim.
• Melhorada a cobertura de localização do instalador: o setup.exe agora inclui também checo, polaco, francês e sérvio, enquanto o MSI permanece como um único pacote en-US para evitar confusão nas releases.
• Corrigida a limpeza na desinstalação das entradas do menu de contexto: "Abrir com Sonarpad" agora é removido de forma confiável, inclusive em cenários legados de registo.
• Corrigida a fiabilidade de pausar/retomar no SAPI5: a pausa com F4 agora funciona corretamente e, ao retomar, volta ao ponto esperado em vez de reiniciar do início.
• Corrigido o fluxo pausar + procurar + retomar na reprodução multimédia: após pausar e avançar/recuar com Esquerda/Direita, ao premir Espaço a reprodução retoma de forma fiável na posição atual em vez de parar ou reiniciar do início.

Versão 0.6.5 – 2026-02-07
Melhorias
• Tradução em espanhol aprimorada graças a Arturo Fernandez Rivas.
• Adicionada uma opção para dividir audiolivros EPUB por capítulos.
• As importações RSS agora usam uma aba temporária dedicada (título localizado); Salvar como a converte em um documento normal.
• As mensagens do leitor de ecrã agora também são enviadas ao JAWS quando disponível.
Correções de bugs
• A leitura a partir do cursor (F5) agora começa exatamente no cursor. Antes podia começar algumas linhas acima porque o deslocamento do cursor não correspondia às posições CRLF/UTF-16.
• Corrigido um problema de redesenho: ao digitar sobre uma seleção, o texto anterior podia desaparecer até mover a seleção.
• Corrigido o parser de capítulos EPUB: páginas de capa ou apenas com imagens não geram mais leitura de CSS (ex.: "padding") nem títulos "Sconosciuto".
• Corrigida a falha ao dividir por tempo audiolivros a partir de EPUB: o Edge TTS podia falhar com chunks vazios ou muito longos ("Edge audio not sent").
• Os artigos RSS agora decodificam entidades HTML (por ex. &quot;, &amp;, &lt;, &gt;).
• Salvar/Salvar como agora sugere o nome do arquivo existente ao salvar formatos que não devem ser sobrescritos (ex.: EPUB), em vez da primeira linha.
• Corrigido um problema em que podcasts com novos episódios não eram anunciados como não reproduzidos, e renomeado "Não ouvido" para "Não reproduzido" por ser mais profissional.

Versão 0.6.4 – 2026-02-05
Melhorias
• O programa foi renomeado para Sonarpad para dar maior ênfase ao som e ao áudio, que são a chave deste programa.
• Adicionada a seleção de faixas de áudio no menu Reprodução para arquivos multimídia com múltiplas faixas de áudio (ex. MKV com vários idiomas).
• Os podcasts agora indicam claramente os não ouvidos com o prefixo "Não ouvido" antes do nome.
• Novo sistema de tags para mudar a voz no texto. Exemplos:
  - Vozes Microsoft (Edge): <voice edge it-IT-IsabellaNeural>Olá</voice>
  - Vozes SAPI5: <voice sapi5 Microsoft Helena Desktop>Olá</voice>
  - Vozes SAPI4: <voice sapi4 #1>Olá</voice>
  - Com velocidade/tom/volume: <voice edge it-IT-ElsaNeural speed=-20 pitch=-5 volume=-10>Olá</voice>
• Categorias de podcasts enriquecidas.
• Adicionada uma opção no menu de contexto para criar um audiolivro a partir da seleção.
• Adicionada a divisão de audiolivros por duração, com a possibilidade de escolher o nome do primeiro arquivo.
• Rótulo do autor localizado na leitura de artigos (ex.: "por", "by", "di").
• Adicionadas opções de indentação (tabulações/espaços com largura) e Tab/Shift+Tab para indentar/desindentar linhas selecionadas.
• Corrigida a limpeza de Markdown: agora trata bullets '*' quando a preservação de listas está desativada.
Correções de bugs
• Corrigido um bug em que audiolivros com SAPI4 podiam ser criados de forma diferente do esperado.
• Janela Buscar em arquivos: ao pressionar Enter em um resultado agora abre na posição correta do trecho e Esc volta aos resultados.
• Janela Opções: ajustado o layout visual das abas Geral, Voz, Editor e Áudio para evitar controles ausentes ou cortados.
• Corrigido um problema de marcadores ao alterar a velocidade de reprodução.
• Corrigido um problema com o Podcast Index e categorias que não eram exibidas corretamente.
• Corrigido o problema do apóstrofo que quebrava a leitura: não há mais leitura separada para diálogos, usam-se tags de voz.

Versão 0.6.3 – 2026-01-30
Melhorias
• Melhorada a detecção do microfone.
• Adicionada reprodução instantânea para todos os formatos.
Correções
• Corrigido o travamento na janela de categorias de podcasts.

Versão 0.6.2 – 2026-01-30
Novas funcionalidades
• Adicionado suporte à execução de arquivos (Shift+F5). Os usuários podem selecionar um interpretador (ex. python) nas Opções, procurá-lo no computador, e pressionando Shift+F5 o script atual é executado. Arquivos HTML abrem no navegador.
• Adicionado suporte para arquivos de ponteiro do Google Docs (.gdoc, .gsheet, .gslides), que abrem automaticamente no navegador padrão.
• Adicionado suporte para o formato de audiolivro M4B (Apple/AAC).
• Adicionada a opção "Mostrar episódios" no menu de contexto dos resultados de pesquisa de podcasts para navegar e reproduzir episódios sem se inscrever.
• Adicionada a função "Ir para linha" (menu Editar ou Ctrl+J) para pular rapidamente para um número de linha específico.
• Adicionadas opções no menu de contexto para ordenar feeds RSS e podcasts (alfabeticamente ou por data).
• Adicionados feeds RSS padrão em vietnamita.
• Adicionada uma caixa de teste de microfone no diálogo de gravação para verificar os níveis antes de começar.
• Adicionada "Mostrar descrição" para episódios de podcast no menu de contexto.
• Adicionado suporte para formatos de áudio/vídeo estendidos via FFmpeg: mkv, avi, mov, m4v, webm, mpg, ts, wmv, flv, vob, 3gp, flac, ogg, wma, aiff.
• Adicionada leitura sincronizada de legendas (srt, vtt, ass, sub, sbv, lrc, smi) com NVDA ou voz selecionada. O programa procura um arquivo de legendas com o mesmo nome do arquivo de mídia. Adicionadas as opções "Importar legendas" e "Remover legendas" no menu Reprodução para arquivos com nomes diferentes.
• Adicionadas associações de arquivos para todos os novos formatos de áudio/vídeo suportados no menu de contexto "Abrir com Sonarpad".
• Adicionada configuração para ajustar o pitch de qualquer arquivo.
• Adicionada opção nas Configurações Gerais para ativar ou desativar relatórios de erros anônimos. Adicionada uma entrada no menu Ajuda para criar um arquivo ZIP de diagnóstico.
• Adicionada opção para usar uma voz diferente para diálogos, tanto para leitura ao vivo quanto para criação de audiolivros.
• Adicionado o navegador de categorias de podcasts para explorar podcasts por categoria (negócios, arte, esportes, etc.).
Melhorias
• Abrir um arquivo de áudio/vídeo do Explorador agora abre diretamente a visualização do reprodutor em vez do editor de texto.
• Removida a solicitação de OCR para PDFs não acessíveis; o OCR agora é realizado automaticamente para melhorar a velocidade e experiência do usuário.
• Melhorado o Terminal Acessível: a leitura NVDA agora lembra a última linha lida para melhor continuidade.
• SAPI 4: A criação de audiolivros agora está totalmente paralelizada e é quase instantânea. Adicionada uma solicitação para escolher o número de processos simultâneos.
• SAPI 4: Eliminado o gargalo WAV-MP3 convertendo fragmentos em paralelo durante a síntese.
• SAPI 4: Melhorado o tratamento de erros e limpeza automática de arquivos temporários.
• Diálogo Localizar: Renomeado "Regex" para "Expressão regular" para maior clareza e adicionadas as traduções ausentes para as opções de pesquisa.
• Audiolivros M4B: Melhor tratamento de saída; dividir por partes/marcadores agora produz um único arquivo M4B com metadados de capítulos incluindo título e autor.
• Reprodutor: Corrigida a precisão de marcadores e anúncios de tempo quando a velocidade de reprodução não é 1.0x.
• Restaurada a navegação Ctrl+Tab e Ctrl+Shift+Tab nas Opções.
• Adicionada uma opção no menu Reprodução para redefinir instantaneamente a velocidade para Normal (1.0x).
• Atualizadas todas as dependências para as versões mais recentes para melhor desempenho e estabilidade.
• Integrado FFmpeg com carregamento dinâmico de DLL para garantir compatibilidade sem bloquear a inicialização.
• Atualizados os filtros de download de podcasts para incluir os novos formatos de áudio/vídeo.
• Impedido que Ctrl+S salve arquivos de áudio/vídeo para evitar corrupção.
• Melhorada a importação de transcrições do YouTube tornando-a mais robusta e resiliente.
• Melhorada a robustez da divisão em partes de audiolivros, garantindo que nenhum texto seja perdido.
• O instalador agora é totalmente multilíngue, suportando Italiano, Inglês, Espanhol, Português, Sueco e Vietnamita com base no idioma do sistema do usuário. O inglês é o padrão para sistemas não suportados.
• Categorias de podcasts: pressionar Enter em uma categoria agora confirma a seleção (equivalente ao botão OK).
• Melhorado o sistema de detecção de travamentos para evitar falsos positivos quando há diálogos modais abertos (mensagens de erro, "texto não encontrado").
Correções
• Corrigido um erro em que o changelog não abria na inicialização.
• Corrigido um erro em que a solicitação de OCR não aparecia para PDFs não acessíveis abertos do Explorador.
• Corrigido um erro de inicialização que podia causar perda de foco ou fechamento de janelas imediatamente após abrir.
• Corrigido um erro crítico na pesquisa regex que impedia encontrar texto, incluindo problemas com "Pesquisa circular" e a opção "Ponto equivale a nova linha" com terminações de linha do Windows.
Localização
• Adicionada a tradução para polonês.
• Adicionada a tradução para francês.
• Adicionada a tradução para tcheco (graças a Radek Žalud e Jiri Holzinger).

Versão 0.6.1 – 2026-01-20
Correções
• Corrigido um erro em que, ao ativar “Mostrar vozes no editor” e reproduzir um podcast, a reprodução era interrompida.
• Corrigido um problema em que alguns podcasts não podiam ser adicionados por URL porque o endereço era truncado.
• Corrigido um erro que impedia a adição de URLs normais na funcionalidade de feeds RSS.
• Corrigido um problema em que o idioma da Wikipedia era mostrado em várias abas das opções.
• Removida a criação de alguns ficheiros de depuração que eram gerados mesmo em modo release.
Melhorias
• Melhorado o suporte para vozes Microsoft, que agora são reproduzidas utilizando um método dedicado com um user agent diferente.
• Adicionado suporte para ficheiros MP4.

Versão 0.6.0 – 2026-01-XX
Melhorias
• Melhorado o suporte para vozes Microsoft, que agora são reproduzidas utilizando um método dedicado com um user agent diferente.

Versão 0.6.0 – 2026-01-20
Novas funcionalidades
• Adicionado o corretor ortográfico. A partir do menu contextual, é possível verificar se a palavra atual está correta e, caso não esteja, obter sugestões.
• Adicionada a importação e exportação de podcasts por meio de arquivos OPML.
• Adicionado suporte à pesquisa no Podcast Index além do iTunes. O utilizador pode introduzir a sua API key e API secret gratuitos (gerados apenas com o seu endereço de e-mail).
• Adicionado suporte às vozes SAPI4, tanto para leitura em tempo real como para a criação de audiolivros
• Adicionado um fallback automático de OCR para PDFs não acessíveis: quando não é encontrado texto extraível, o documento é reconhecido através de OCR..
• Adicionado suporte de dicionário através do Wiktionary. Ao pressionar a tecla Aplicações, são apresentadas as definições e, quando disponíveis, também sinónimos e traduções para outros idiomas.
• Adicionada a importação de artigos da Wikipedia com pesquisa, seleção de resultados e importação direta para o editor.
• Adicionado o atalho Shift+Enter no módulo RSS para abrir um artigo diretamente no site original.
Melhorias
• A seleção do microfone agora é sempre respeitada pela aplicação.
• Na janela de podcasts, ao pressionar Enter num episódio, o NVDA anuncia imediatamente “a carregar”, fornecendo confirmação imediata da ação.
• Nos resultados de pesquisa de podcasts, ao pressionar Enter, o utilizador passa a subscrever o podcast selecionado.
• Corrigidas e melhoradas as etiquetas dos atalhos Ctrl+Shift+O e Podcast Ctrl+Shift+P.
• A velocidade de reprodução e o volume passam agora a ser guardados nas definições e mantêm-se para todos os ficheiros de áudio.
• Adicionada uma pasta de cache dedicada para os episódios de podcasts. O utilizador pode conservar os episódios através de “Conservar podcast” no menu Reproduzir. A cache é limpa automaticamente quando ultrapassa o tamanho definido pelo utilizador (Opções → Áudio).
• Melhorada de forma significativa a obtenção de artigos RSS utilizando libcurl com impersonação de Chrome e iPhone, garantindo compatibilidade com cerca de 99 % dos sites.
• Adicionado o estado lido / não lido para os artigos RSS, com indicação clara na lista RSS.
• A função Substituir tudo agora mostra também o número de substituições efetuadas.
• Adicionado o botão Eliminar podcast ao navegar pela biblioteca de podcasts através da tecla Tab.
Correções
• Removida a entrada redundante “pending update” do menu Ajuda (as atualizações já são geridas automaticamente).
• Corrigido um erro em que, ao abrir um ficheiro MP3 e pressionar Ctrl+S, o ficheiro era guardado e ficava corrompido.
• Corrigido um problema de interface em que “Batch Audiobooks” era apresentado como “(B)… Ctrl+Shift+B” (removida a etiqueta redundante).
• Corrigido o funcionamento das aspas inteligentes: quando ativadas, as aspas normais passam agora a ser corretamente substituídas por aspas tipográficas.
• Corrigido um erro em que, ao utilizar “Ir para o marcador”, a velocidade de reprodução era reposta para 1.0.
• Corrigido um problema em que episódios de podcasts já descarregados eram novamente descarregados em vez de ser utilizada a versão em cache.
Atalhos de teclado
• F1 agora abre o guia.
• F2 agora verifica a existência de atualizações.
• F7 / F8 agora permitem navegar para o erro ortográfico anterior ou seguinte.
• F9 / F10 agora permitem alternar rapidamente entre as vozes guardadas nos favoritos.
Melhorias para desenvolvedores
• Os erros deixaram de ser ignorados silenciosamente: todos os padrões let _ = foram removidos e os erros são agora tratados explicitamente (propagados, registados ou tratados com mecanismos de fallback adequados).
• O projeto agora não compila se existirem avisos: tanto cargo check como cargo clippy devem passar sem warnings, com lints mais restritivos e remoção de allow sempre que possível.
• Removidas implementações personalizadas do tipo strlen / wcslen. Os comprimentos de strings e buffers UTF-16 passam agora a ser derivados de dados geridos pelo Rust, sem varrimentos manuais de memória.
• A gestão de DLL foi limpa e consolidada em torno de libloading, evitando lógica de carregamento personalizada e parsing PE.
• Removidos os helpers manuais de parsing de bytes: todo o parsing passa agora a utilizar from_le_bytes / from_be_bytes sobre slices verificadas.
Estas alterações reduzem o uso desnecessário de unsafe, eliminam potenciais comportamentos indefinidos e tornam a base de código mais idiomática, robusta e fácil de manter.

Versao 0.5.9 - 2026-01-13
Novas funcionalidades
• Adicionada a possibilidade de reordenar RSS pelo menu contextual (cima/baixo/posicao), com validacao de posicoes invalidas.
• Adicionado menu contextual para artigos com abrir site original e compartilhar via WhatsApp, Facebook e X.
• Adicionado atalho Esc para voltar de artigos importados para a lista de RSS.
• Adicionada a modalidade podcast: buscar, inscrever e ouvir; reordenar assinaturas; Esc para parar a reproducao e voltar a lista; Enter em um episodio inicia a reproducao.
• Adicionado controle de velocidade de reproducao para podcasts e arquivos MP3.
• Adicionado Ctrl+T para ir a um tempo especifico.
• Adicionado um botao de previa de voz apos o combo de volume.
• Adicionada a funcao regex para Localizar e Substituir, estilo Notepad++.
• Adicionada a importacao de RSS a partir de arquivos OPML e TXT.
• Adicionada nas Opcoes a caixa para habilitar "Abrir com Sonarpad" no Explorador de arquivos, inclusive na versao portable.
Melhorias
• Melhorada a selecao de velocidade, tom e volume das vozes, respeitando os limites maximos do TTS.
• Varias melhorias no RSS para baixar todos os artigos sem mover o foco do NVDA durante atualizacoes.
• Melhorada a reproducao de audio com um menu dedicado, anuncio de tempo com Ctrl+I e volume ate 300%.
• Adicionados atalhos faltantes para algumas funcoes.
• Reorganizado o menu Editar com um submenu para as funcoes de limpeza de texto.
• Reorganizadas as Opcoes em abas, com Ctrl+Tab e Ctrl+Shift+Tab para navegar.
• Resolvidos os problemas de leitura de artigos: o leitor RSS agora mostra os artigos completos como no navegador.
Correcoes
• Corrigido um problema em que a limpeza de Markdown removia numeros no inicio da linha.
• Corrigido AltGr+Z que acionava Undo.
• Corrigido um problema em que ao gravar um audiolivro nao era possivel interromper rapidamente.
Localizacao
• Adicionada a traducao vietnamita (graças a Anh Duc Nguyen).

Versao 0.5.8 - 2026-01-10
Novas funcionalidades
• Adicionado controle de volume para o microfone e o audio do sistema ao gravar podcasts.
• Adicionada uma nova funcao para importar artigos de sites ou feeds RSS, incluindo os feeds mais importantes para cada idioma.
• Adicionada uma funcao para remover todos os marcadores do arquivo atual.
• Adicionada a funcao para remover linhas duplicadas e linhas duplicadas consecutivas.
• Adicionada a funcao para fechar todas as abas ou janelas exceto a atual.
• Adicionada a entrada Doacoes no menu Ajuda para todos os idiomas.
Melhorias
• Melhorado o terminal acessivel para evitar alguns crashes.
• Melhoradas e corrigidas as access key e os atalhos de teclado do programa.
• Corrigido um problema em que, ao fechar a janela de reproducao de audio, a reproducao nao parava.
• Adicionadas janelas de confirmacao para acoes importantes (ex.: remover linhas duplicadas, remover hifens no fim da linha, remover todos os marcadores do arquivo atual). Nenhuma confirmacao e mostrada se a acao nao se aplica.
• Adicionada a possibilidade de excluir feeds/sites RSS da biblioteca selecionando-os e pressionando Delete.
• Adicionado um menu contextual na janela RSS para modificar ou eliminar feeds/sites RSS.
• Removida a opcao para mover as definicoes para a pasta atual; agora o programa faz isso automaticamente (se a pasta do exe se chama "sonarpad portable" ou o exe esta em unidade removivel, salva na pasta do exe em `config`, senao em `%APPDATA%\\Sonarpad`, com fallback para `config` se a pasta preferida nao for gravavel).

Versao 0.5.7 - 2026-01-05
Novas funcionalidades
• Adicionada a opcao para gravar audiolivros em lote (conversao multipla de arquivos e pastas).
• Adicionado suporte para arquivos Markdown (.md).
• Adicionada a escolha da codificacao ao abrir arquivos de texto.
• Adicionada opcao no terminal para anunciar novas linhas com NVDA.
Melhorias
• A gravacao de audiolivros agora e salva em MP3 nativo quando selecionado.
• O usuario pode escolher onde inserir o asterisco * que indica modificacoes nao salvas.
• Melhorado o sistema de atualizacao para ser mais robusto em diferentes cenarios.
• Adicionada no menu Editar a funcao para remover hifens no final da linha (util para textos OCR).

Versao 0.5.6 - 2026-01-04
Correcoes
  Melhorado Procurar em arquivos: ao pressionar Enter abre o arquivo exatamente no trecho selecionado.
Melhorias
  Suporte a PPT/PPTX.
  Para formatos nao textuais, Salvar agora propoe sempre .txt para evitar corromper a formatacao (PDF/DOC/DOCX/EPUB/HTML/PPT/PPTX).
  Gravacao de podcast do microfone e/ou audio do sistema (menu Arquivo, Ctrl+Shift+R).

Versao 0.5.5 - 2026-01-03
Novas funcionalidades
• Adicionado um terminal acessivel otimizado para muita saida e leitores de tela (Ctrl+Shift+P).
• Adicionada a opcao de guardar as definicoes na pasta atual (modo portable).
Correcoes
• Melhorados os trechos de Procurar em arquivos para manter a previsualizacao alinhada com a ocorrencia.

Versao 0.5.4 – 2026-01-03
Melhorias
• Correcao da funcao Normalizar espacos em branco (Ctrl+Shift+Enter).
• Suporte a HTML/HTM (abrir como texto).

Versao 0.5.3 – 2026-01-02
Novos recursos
• Adicionado Buscar em arquivos.
• Adicionadas novas ferramentas de texto: Normalizar espacos em branco, Quebra de linha dura e Remover Markdown.
• Adicionadas Estatisticas de texto (Alt+Y).
• Adicionados novos comandos de lista no menu Editar:
• Ordenar itens (Alt+Shift+O)
• Manter itens unicos (Alt+Shift+K)
• Inverter itens (Alt+Shift+Z)
• Adicionados Comentar / Descomentar linhas (Ctrl+Q / Ctrl+Shift+Q).
Localizacao
• Adicionada a localizacao em espanhol.
• Adicionada a localizacao em portugues.
Melhorias
• Quando um arquivo EPUB esta aberto, Salvar muda automaticamente para Salvar como e exporta o conteudo como .txt para evitar corromper o EPUB.

## 0.5.2 - 2026-01-01
- Adicionado um changelog.
- Adicionadas opcoes "Abrir com Sonarpad" e associacoes de arquivos suportados durante a instalacao.
- Melhorada a localizacao de mensagens (erros, dialogos, exportacao de audiolivro).
- Adicionada a selecao de partes ao usar "Dividir audiolivro por texto", com a opcao "Exigir o marcador no inicio da linha".
- Adicionada a importacao de transcricoes do YouTube com selecao de idioma, opcao de timestamps e melhorias de foco.

## 0.5.1 - 2025-12-31
- Atualizacoes automaticas com confirmacao, melhorias de erros e notificacoes.
- Melhorias na exportacao de audiolivros (divisao por texto, SAPI5/Media Foundation, controles avancados).
- Melhorias em TTS (pausa/retomar, dicionario de substituicoes, favoritos).
- Menu Ver e paineis de vozes/favoritos, cor e tamanho de texto.
- Idioma padrao do sistema e melhorias de localizacao.
- CI e empacotamento Windows (artefatos, MSI/NSIS, cache).

## 0.5.0 - 2025-12-27
- Refatoracao modular (editor, manipulacao de arquivos, menu, busca).
- Workflow de compilacao/empacotamento Windows e atualizacoes de README/licenca.
- Correcao da navegacao TAB na janela de Ajuda.

## 0.5 - 2025-12-27
- Atualizacao preliminar da versao.

## 0.1.0 - 2025-12-25
- Versao inicial: estrutura do projeto e README.

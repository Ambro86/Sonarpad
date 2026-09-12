# Changelog

Versão 0.9.8 – 2026-09-12

Audiodescrição com IA
1. Adicionada uma recuperação conservadora para exportação multicanal sem alterar o pipeline dos arquivos que já funcionam. O Sonarpad continua usando primeiro o layout de canais original e só completa um último quadro PCM incompleto quando necessário. Se o WAV intermediário multicanal ainda falhar porque o número de amostras não é múltiplo do número de canais, o Sonarpad tenta novamente apenas essa exportação em estéreo. Exportações mono, estéreo e multicanal que já funcionam permanecem inalteradas.

Tema escuro e acessibilidade
1. Corrigido um problema em que o tema escuro interferia nas caixas de diálogo nativas do Windows e nas mensagens de confirmação. O Sonarpad não aplica mais seu subclass personalizado de tema escuro às árvores de diálogo do sistema `#32770`, por isso Abrir/Salvar, a exportação de diagnósticos e MessageBox continuam sob gerenciamento do Windows e recebem corretamente o foco do teclado/NVDA. Também foi corrigido o tempo de vida da extensão ZIP padrão no diálogo Salvar dos diagnósticos.

Versão 0.9.7 – 2026-09-11

Audiodescrição com IA
1. Adicionada a opção opcional para criar a audiodescrição no arquivo de vídeo original. Quando ativada, o Sonarpad agora prefere MP4: copia o fluxo de vídeo original sem recodificação e adiciona a faixa de audiodescrição mixada. Se o FFmpeg indicar incompatibilidade do contêiner ou da gravação de pacotes ao criar o MP4, o Sonarpad tenta automaticamente a mesma exportação em MKV, também sem recodificar o vídeo. Também é possível escolher MKV explicitamente. Nesse modo, as pausas estendidas são ignoradas para manter áudio e vídeo sincronizados.

2. Melhorada a reanálise de segmentos nos projetos de audiodescrição salvos. A janela agora usa o acesso à IA já configurado na janela principal Criar audiodescrição, sem duplicar chave de API, crédito do Sonarpad AI e seleção de modelo. Um segmento reanalisado agora mantém todo o grupo de descrições do projeto pertencentes ao mesmo trecho de análise, em vez de apenas a primeira ou a mais próxima; todas as descrições do grupo são marcadas como reanalisadas e “Aplicar segmento e reexportar MP3” aplica o grupo inteiro de uma só vez. As prévias das pausas estendidas agora são sintetizadas diretamente em vez de buscar a posição no MP3 exportado, evitando prévias que comecem com o áudio do filme ou cortem a descrição.

3. O fallback conservador de mídia do Gemini foi aprimorado sem alterar o fluxo que já funciona. O HTTP 400 `INVALID_ARGUMENT` continua acionando trechos MKV menores (cerca de 15 MB) e depois trechos MP4 compatíveis. Além disso, se o Gemini aceitar um trecho enviado, mas o processamento terminar em `FAILED`, o Sonarpad envia exatamente o mesmo trecho novamente uma vez e só então passa para o MP4; as tentativas do Code 13 do servidor ficam limitadas a três antes do mesmo fallback, evitando esperas infinitas. Erros de rede, cota, autenticação ou síntese não acionam esse caminho.

4. Corrigida a persistência das credenciais de IA ao mudar o modo de acesso. A chave de API pessoal do Gemini e o código de acesso do Sonarpad AI agora são salvos de forma independente: alternar entre Chave de API pessoal e Sonarpad AI não apaga mais a credencial inativa. A chave Gemini também é salva quando o campo perde o foco.

5. Foi adicionado um verdadeiro botão Cancelar a “Reanalisar segmento” e “Aplicar segmento e reexportar MP3”. A barra de progresso continua ativa e Cancelar agora interrompe a preparação do segmento pelo FFmpeg, o worker de IA, a validação TTS e a exportação final assim que a etapa atual puder parar com segurança. A aplicação de um segmento reanalisado agora é transacional: o projeto e o arquivo de mídia existentes só são substituídos após uma reexportação bem-sucedida; se a operação for cancelada, os arquivos anteriores permanecem inalterados e a proposta reanalisada continua disponível para uma nova tentativa.

6. Corrigida a reanálise de segmentos para descrições fora do primeiro chunk do Gemini. O segmento isolado agora é enviado ao worker existente com uma linha do tempo local iniciada em 0 e depois remapeado para a posição absoluta no projeto, evitando `Invalid prepared chunk timeline at chunk 1` sem alterar o fluxo normal de audiodescrição.

7. A reanálise de segmentos em projetos salvos foi refeita para usar os mesmos controles de segurança da criação completa de audiodescrição. O trecho selecionado passa agora pela mesma extração de áudio mono a 16 kHz e análise de diálogos/silêncios com Pyannote, pelas mesmas regras temporais e fallbacks do Gemini, pela mesma síntese TTS real com a voz do projeto e pelo mesmo agendador final para posicionamento seguro e pausas estendidas. Ao aplicar o segmento, o Sonarpad mantém os novos tempos absolutos já verificados e atualiza a proteção Pyannote desse trecho; foi removida a solução especial anterior da prévia para textos reanalisados longos demais.

8. Corrigida a substituição estrutural do segmento após a reanálise completa. O Sonarpad não exige mais que o segmento recém-verificado tenha exatamente o mesmo número de descrições do projeto salvo: as descrições antigas daquele chunk são substituídas pelo resultado seguro completo da pipeline, portanto um segmento com 10 descrições pode corretamente passar a 9 (ou 11). O editor mostra imediatamente a nova quantidade e os novos tempos para prévia, enquanto o projeto e o arquivo de mídia salvos permanecem inalterados até “Aplicar segmento e reexportar MP3” terminar com sucesso.

9. A reanálise de segmentos foi reformulada para tratar o trecho físico selecionado como um verdadeiro minifilme independente. O Sonarpad passa agora esse minifilme diretamente para exatamente a mesma função `create_audio_description` usada na criação completa normal, para que vídeo e faixa de áudio compartilhem a mesma linha do tempo local e os controles normais de Pyannote, Gemini, TTS real, agendamento e segurança sejam executados sem uma implementação separada de reanálise. Somente depois que esse fluxo normal termina os tempos locais resultantes são deslocados de volta para a posição original no filme.

Edição de texto
1. Melhorado “Unir linhas quebradas” em Editar > Texto. Sem seleção, processa o documento inteiro; com uma seleção, atua apenas nas linhas selecionadas. Agora reconstrói parágrafos completos unindo linhas consecutivas mesmo quando uma frase termina com pontuação; linhas em branco continuam separando parágrafos e listas numeradas ou com marcadores são preservadas.

10. Corrigida a sincronização temporal da reanálise ao mapear o mini-filme analisado de volta para o filme original. O Sonarpad agora aplica a mesma pequena reconciliação da duração dos segmentos usada na análise completa às descrições, aos tempos de Visual Evidence e aos intervalos de diálogo protegidos, evitando desvios progressivos de frações de segundo sobre a fala.

Versão 0.9.6 – 2026-09-09

1. Corrigidos os travamentos de algumas vozes SAPI4 com o acompanhamento do cursor ativado. O bridge agora aguarda a conclusão real da síntese, mantendo o acompanhamento ativo.

2. Corrigidos a sincronização entre microfone e áudio do sistema e os estalos causados pelo arredondamento dos registros de tempo na gravação de podcasts. As correções também valem ao salvar as fontes separadamente.

3. A síntese SAPI5 das audiodescrições agora utiliza processos separados. Em caso de falha, tenta novamente com menos paralelismo, preserva as descrições concluídas e memoriza o limite funcional para a voz.

Versão 0.9.5 – 2026-09-07

Audiodescrição com IA
1. Melhorada a resiliência quando o Gemini retorna temporariamente `MALFORMED_RESPONSE` sem conteúdo utilizável. O Sonarpad agora tenta novamente exatamente o mesmo segmento até três vezes, com uma breve espera entre as tentativas, em vez de interromper imediatamente toda a audiodescrição. As respostas normais do Gemini, os bloqueios de segurança, os erros de limite de tokens e o comportamento atual dos pontos de controle permanecem inalterados.

2. Corrigido outro caso raro de `invalid Gemini chunk timeline` em alguns vídeos cujos segmentos preparados pelo FFmpeg apresentam um pequeno desvio acumulado de duração. O Sonarpad mantém o fluxo normal inalterado quando a linha do tempo é válida e usa um reajuste conservador apenas quando a linha do tempo normal falharia e a diferença total é pequena; diferenças grandes ou suspeitas continuam gerando erro.

3. Adicionado o serviço opcional Sonarpad AI para audiodescrições com IA. Os usuários podem continuar usando sua própria chave API Gemini ou escolher o serviço Sonarpad e inserir um código Sonarpad pessoal. O código é protegido com o Windows DPAPI. No modo de serviço, os trechos de vídeo preparados são enviados diretamente do PC do usuário para o Google por meio de uma URL temporária de upload do Google; sonarpad.com não recebe nem armazena os bytes do vídeo. O backend apenas autoriza o acesso, impõe o modelo Gemini/nível Standard e os limites de uso, contabiliza o consumo e devolve a resposta do Gemini.

4. Melhorada a edição de projetos de audiodescrição salvos. Agora é possível alterar várias descrições em sequência sem precisar aplicar cada mudança imediatamente. As alterações ficam na memória; ao pressionar “Aplicar texto”, o Sonarpad verifica cada descrição modificada em relação ao seu intervalo salvo e depois salva juntas todas as alterações válidas. Se uma descrição não couber no intervalo, o foco volta para ela sem perder as outras alterações pendentes.

5. Adicionado “Ajustar voz” imediatamente antes de “Criar audiodescrição”. A nova janela permite escolher mecanismo, voz, velocidade e volume e inclui “Testar voz”. Ao pressionar OK, o foco volta exatamente para “Ajustar voz” e os valores escolhidos são aplicados à audiodescrição que será criada. Se a janela não for usada, a síntese permanece exatamente como antes.


6. Foram ampliados os controles de voz nos projetos de audiodescrição salvos. Em Modificar projeto, além do mecanismo e da voz, agora também estão disponíveis velocidade, volume e “Testar voz”. Ao aplicar os parâmetros de voz, o Sonarpad verifica novamente todas as descrições existentes com os novos parâmetros de síntese e só reconstrói o MP3 se todas continuarem dentro dos respectivos intervalos salvos. Os intervalos sem diálogo, os tempos de Visual Evidence, as descrições do Gemini e a análise temporal do projeto são reutilizados sem executar novamente a análise de IA.

Versão 0.9.4 – 2026-09-04

Audiodescrição com IA
1. Corrigido um problema com vídeos Matroska/WebM que contêm uma marca de tempo inicial interna anormalmente alta e que podia interromper a geração da audiodescrição com o erro “invalid Gemini chunk timeline”. O Sonarpad agora normaliza a duração do arquivo de origem e dos segmentos Gemini somente quando esse padrão anômalo de marcas de tempo é detectado, mantendo inalterados os vídeos normais e o comportamento da audiodescrição que já funcionava.
2. Melhorado o ducking da audiodescrição para obter uma mixagem mais natural. O áudio original agora abaixa suavemente antes de a voz começar e retorna de forma gradual ao nível normal depois da descrição; quando há descrições muito próximas, o nível de fundo permanece estável, evitando quedas e subidas repetidas de volume.
3. Corrigido um problema que podia fazer a exportação final da audiodescrição falhar com alguns arquivos AVI e outras fontes com áudio 5.1/multicanal, quando a decodificação terminava com um quadro de áudio intercalado incompleto. O Sonarpad agora completa com segurança somente esse último quadro parcial com a quantidade estritamente necessária de amostras silenciosas; os quadros já completos e os arquivos mono, estéreo ou multicanal normais permanecem inalterados.


Versão 0.9.3 – 2026-09-03

Vozes SAPI5
1. Corrigido um problema em que algumas vozes SAPI5 locais podiam não falar com o movimento do cursor ativado, durante a leitura com várias vozes ou ao criar audiolivros/MP3. O Sonarpad agora usa um caminho de síntese SAPI5 para arquivo que funciona de forma confiável no Windows e continua permitindo cancelar a síntese, enquanto a reprodução direta normal permanece inalterada.
2. Corrigida a posição do cursor durante a leitura de diálogos com várias vozes. As etiquetas de voz inseridas automaticamente pelo Sonarpad para os diálogos agora são tratadas apenas como metadados de reprodução e não como caracteres no editor, evitando que após F4 ou F6 o cursor avance além do texto real. A leitura com uma única voz e as etiquetas <voice> escritas explicitamente nos documentos permanecem inalteradas.

Audiodescrição com IA
1. Adicionada a caixa “Mostrar chave API” logo após o campo da chave API Gemini. Ela fica desmarcada por padrão; quando ativada, mostra temporariamente a chave completa para permitir verificar se ela foi colada por inteiro. Ao reabrir a janela, a chave volta a ficar oculta.

Podcasts e Wikipédia
1. Depois de salvar uma gravação de podcast, o Sonarpad agora pergunta se deseja abrir a pasta que contém o arquivo salvo, como já acontece ao salvar mídias do YouTube/streaming.
2. Ao importar outro artigo da Wikipédia para um editor que já contém texto, o novo artigo agora é acrescentado ao final em vez de ser inserido no início. O cursor é posicionado no começo do artigo recém-importado.


Versão 0.9.2 – 2026-09-02

Audiodescrição com IA
1. Corrigido um problema que podia fazer a audiodescrição com IA falhar durante a exportação final para MP3 em vídeos com áudio multicanal, como 5.1. O Sonarpad agora converte automaticamente o áudio multicanal para estéreo somente quando necessário para a codificação MP3, sem alterar as exportações mono ou estéreo.
2. Ao iniciar a Audiodescrição com IA em um vídeo com várias faixas de áudio, o Sonarpad agora pergunta qual faixa deve ser usada antes do processamento. A caixa de combinação acessível pode ser alterada com as setas; OK inicia a audiodescrição com a faixa selecionada, enquanto Cancelar fecha a janela de audiodescrição e devolve o foco ao editor do Sonarpad.

YouTube e streaming
1. Corrigido um problema em que iniciar a audiodescrição com IA em um vídeo da página 2 ou posterior de uma playlist ou canal do YouTube podia reabrir a janela de seleção do YouTube e tirar o foco da janela de audiodescrição. O Sonarpad agora fecha corretamente o seletor sem voltar às páginas anteriores.
Versão 0.9.1 – 2026-09-01

Downloads do YouTube
• Corrigido um problema que fazia as janelas de progresso dos downloads do YouTube/streaming voltarem repetidamente ao primeiro plano depois de alternar para outro aplicativo com Alt+Tab. Os downloads agora continuam em segundo plano sem roubar o foco.
• Melhorada a acessibilidade do progresso dos downloads. Ao voltar para a janela de progresso, os leitores de tela podem ler o status atual e a porcentagem. Em playlists, o Sonarpad também informa o número do item atual, o total de itens e o título.
• Corrigidos falsos avisos de travamento do watchdog durante downloads e conversões longas quando a janela de progresso continuava respondendo.
• Foi adicionada uma caixa de combinação Formato aos downloads de playlists. Na lista de vídeos, pressione Tab para escolher MP4, MP3, M4A, OPUS, OGG, WAV ou FLAC antes de iniciar o download múltiplo.
• O salvamento de mídia em streaming foi reorganizado. O formato e a qualidade agora são escolhidos no momento de salvar, em vez de na janela inicial de pesquisa de streaming. “Salvar mídia” abre uma única janela de Formato e Qualidade, e os downloads de playlists oferecem as duas caixas de combinação.

Audiodescrição com IA
• Corrigido um problema que podia impedir o início da audiodescrição com IA em alguns vídeos MKV. O Sonarpad agora lida de forma mais confiável com vídeos que têm marcas de tempo irregulares ou ausentes.

Versão 0.9.0 – 2026-08-31

Audiodescrição com IA — nova função principal
• Foi adicionado “Criar audiodescrição com IA” em Ferramentas > Multimídia. O Sonarpad analisa o áudio para encontrar espaços sem diálogo, gera as descrições com Gemini e usa os mecanismos de voz já disponíveis, evitando falar por cima dos diálogos.
• Foi melhorada a sincronização entre o que acontece no vídeo e as descrições, com verificações automáticas dos tempos gerados pelo Gemini.
• “Ativar pausas estendidas” fica desmarcada por padrão. Ela pode ser ativada em conteúdos com muitos diálogos ou pouco espaço disponível para permitir descrições mais longas.
• O Sonarpad pode tentar reconhecer os personagens e usar seus nomes. Os catálogos de personagens podem ser mantidos entre episódios de uma série para melhorar a continuidade.
• É possível salvar o projeto, editar posteriormente as descrições e exportar novamente sem precisar gerar tudo de novo com Gemini.
• Se o processo for interrompido, o Sonarpad mantém o progresso e permite continuar a audiodescrição. Se a cota do Gemini se esgotar, é possível esperar, trocar de modelo ou interromper sem perder o trabalho já concluído.
• A janela permite escolher idioma, nível de detalhe, modelo Gemini, mecanismo e voz, e lembra as preferências utilizadas.
• O módulo está disponível nos 17 idiomas do Sonarpad. Durante a geração, a interface mostra apenas o progresso, o estado atual e Cancelar; ao final, o MP3 pode ser aberto diretamente no player interno.

E-books e documentos
• Foi adicionada a importação de Kindle sem DRM nos formatos MOBI, AZW e AZW3, com texto e capítulos disponíveis no editor e no índice.
• Foi adicionado suporte a DAISY 2.02 e DAISY 3. Os audiolivros DAISY usam o player interno do Sonarpad e respeitam a navegação e os limites dos capítulos.
• Kindle e DAISY são importados sem sobrescrever o arquivo original; Kindle protegidos por DRM são recusados explicitamente.
• Foi corrigido “Salvar como” para EPUB: ao escolher TXT ou outro formato, agora é usada a extensão selecionada e o EPUB original continua associado ao documento aberto.

RSS e artigos
• Foi adicionada a seleção múltipla de artigos RSS para excluir vários em uma única operação.
• Os RSS agora suportam pastas reais, preservadas durante a importação e exportação OPML, inclusive pastas vazias.
• Os feeds podem ser reordenados dentro da pasta atual com Mover para cima, Mover para baixo, Mover para o início, Mover para o fim e Mover para a posição.

Acessibilidade, guias e interface
• Os guias do Sonarpad foram reorganizados com um índice e foi adicionado um guia completo sobre Audiodescrição com IA.
• Foi corrigido um problema da tradução alemã que podia impedir a exibição de Abrir, Salvar como e outras janelas de seleção de arquivos.

Vozes e idiomas
• O catálogo de vozes Google TTS para download passou de 104 para 156 pacotes e de 53 para 81 variantes de idioma.
• Foram adicionados novos pacotes Google TTS e nomes localizados para mais idiomas em toda a interface.

Versão 0.8.4 – 2026-07-24

Edição de documentos EPUB
• O Sonarpad agora consegue não apenas abrir documentos EPUB, mas também editá-los e salvá-los novamente em formato EPUB, preservando a formatação original, o sumário, as notas de rodapé, as imagens, as folhas de estilo, os metadados e os links internos.
• O formato EPUB está disponível em “Salvar como” para documentos abertos a partir de um EPUB. Ao salvar, apenas o texto alterado é atualizado e a estrutura do livro permanece intacta.

Confiabilidade dos audiolivros
• Corrigido um problema intermitente em que, após cinco tentativas com falha do Google TTS, uma unidade de síntese era descartada silenciosamente e uma parte do texto podia faltar no audiolivro final.
• As unidades do Google agora são repetidas até funcionarem ou até o usuário cancelar. A inicialização dos processos é escalonada para reduzir conflitos temporários com o Chrome e com arquivos; o Sonarpad também interrompe a criação em vez de salvar um audiolivro com um segmento ausente.
• Os audiolivros com Edge agora repetem sem limite fixo os erros temporários de rede, WebSocket, tempo limite, limitação do serviço e áudio inválido, até funcionar ou o usuário cancelar, inclusive com vozes mistas e divisão por duração. SAPI4 e SAPI5 mantêm tentativas adaptativas e limitadas; se um segmento continuar falhando, o Sonarpad interrompe o processo sem salvar um audiolivro incompleto.

Navegação nas bibliotecas digitais
• Os resultados do LibriVox, Internet Archive e Project Gutenberg agora usam navegação por páginas como o YouTube: “Ir para os resultados anteriores” aparece no início da lista e “Ir para os próximos resultados” no final.
• Foram corrigidas as transições de foco no LibriVox: ao abrir um livro ou capítulo, o foco do NVDA não vai mais para o editor principal antes da abertura da próxima lista ou do player.
• Foi adicionada uma proteção de foco durante as pesquisas e o carregamento de livros do LibriVox: uma janela de carregamento traduzida permanece em primeiro plano durante toda a solicitação, impedindo que o foco do NVDA passe para o Prompt de Comando, o Windows Terminal ou outro aplicativo.

Download de playlists do YouTube
• Foi adicionado às playlists do YouTube um comando acessível de seleção múltipla, permitindo escolher quais vídeos baixar sem alterar o comando existente “Salvar mídia” do item em reprodução.
• Os itens selecionados são baixados um de cada vez usando o formato e a qualidade escolhidos ao abrir a playlist, recebem nomes numerados que preservam a ordem original e são salvos em uma pasta própria dentro da pasta Mídia configurada.
• A janela inclui “Selecionar tudo” e “Desmarcar tudo”, anuncia quantos itens estão selecionados, permite cancelar mantendo os arquivos já concluídos e informa claramente os itens que não puderam ser baixados.
• Os itens da playlist agora são caixas de seleção nativas: os leitores de tela anunciam automaticamente o título, o tipo de controle e o estado marcado ou desmarcado, sem acrescentar palavras ao título visível nem usar anúncios de voz forçados.

Versão 0.8.3 – 2026-07-23

Modo escuro
• Adicionado um modo escuro, que pode ser ativado no menu Ver e é salvo nas preferências.
• O tema escuro é aplicado ao editor, aos menus, às janelas auxiliares e aos principais controles, adaptando as cores do texto para manter a legibilidade e a acessibilidade.

Idioma alemão
• Adicionado o alemão como idioma completo da interface, selecionável nas Opções.
• Notícias e RSS, corretor ortográfico, calendário e todas as citações, doações, guia e changelog estão integralmente disponíveis em alemão.

Português (Brasil) e Google Notícias
• Adicionado o Português (Brasil) como idioma completo e independente do Português (Portugal), selecionável nas Opções.
• Toda a interface, o calendário, as 365 efemérides, todas as citações, o corretor ortográfico, as doações, o guia e o registro de alterações estão disponíveis em português brasileiro.
• O Google Notícias usa a localização do Brasil e oferece as categorias Brasil, Mundo, Negócios, Ciência e tecnologia, Entretenimento, Esportes e Saúde.
• Adicionadas fontes RSS brasileiras padrão, mantidas separadamente das fontes portuguesas.
• As notícias do Google Notícias podem mostrar as diferentes fontes relacionadas ao mesmo assunto como itens filhos acessíveis pela árvore.

LibriVox
• A pesquisa do LibriVox foi otimizada para evitar solicitações excessivas ao serviço e travamentos da interface. As varreduras extensas do catálogo foram removidas, o número de tentativas foi reduzido e tempos limite menores foram introduzidos.

Síntese de voz
• As sequências de três ou mais pontos agora são normalizadas antes da leitura, evitando que algumas vozes pronunciem “ponto ponto” ou criem segmentos formados apenas por pontuação.

Artigos relacionados do Google Notícias
• Para cada notícia, quando disponíveis, agora são exibidos artigos relacionados, ou seja, outros artigos que tratam da mesma notícia. Para lê-los, basta expandir o artigo principal quando o Sonarpad informar que há artigos relacionados disponíveis. Quem não quiser expandir essa seção só precisa pressionar Enter no artigo principal e ler a notícia como sempre.
• Os artigos relacionados agora usam o mesmo sistema lido/não lido dos artigos principais, incluindo anúncios acessíveis, data e hora, salvamento do estado e sua conservação após a atualização das fontes ou a reinicialização do Sonarpad.

Anúncios nas partes dos audiolivros
• Foi adicionada às Opções de áudio a caixa de combinação “Anúncio no início de cada parte”. Nos audiolivros divididos em vários arquivos, cada parte pode começar sem anúncio, com o título do livro, o título e o número da parte, o nome do arquivo ou o nome do arquivo e o número da parte.

Versão 0.8.2 – 2026-07-17

Bibliotecas digitais e audiolivros
• Adicionado o Project Gutenberg, com pesquisa por título ou autor e seleção do idioma.
• Os livros EPUB do Project Gutenberg são baixados para Documentos\Sonarpad\Documents; no final, o Sonarpad pergunta se o livro deve ser aberto imediatamente no editor.
• Adicionado o Internet Archive para pesquisar e ouvir coleções de áudio, incluindo programas de rádio antigos, discursos e música ao vivo.
• Adicionado o LibriVox para pesquisar audiolivros por título ou autor e reproduzir diretamente os capítulos com o mesmo leitor utilizado para podcasts.
• As três novas funções estão disponíveis no menu Ferramentas e, quando o agrupamento dos menus está ativo, na seção Leitura.

Transcrições de áudio longas
• Corrigida a transcrição de arquivos de áudio longos: o áudio agora é dividido automaticamente em partes de 15 minutos, transcrito uma parte de cada vez e depois reunido, evitando erros que podiam ocorrer com gravações longas.

YouTube
• As ações mais úteis que antes só estavam disponíveis depois de abrir um vídeo do YouTube e acessar o menu Reprodução agora também estão disponíveis diretamente no menu de contexto do mesmo vídeo, como “Transcrever áudio atual”, “Criar audiodescrição com IA” e “Baixar mídia”, para facilitar o uso.
• Adicionada a opção “Copiar link”, também disponível com Ctrl+C, para copiar para a área de transferência o URL do vídeo, da playlist ou do canal do YouTube selecionado.

Versão 0.8.1 – 2026-07-16

Síntese de voz Google
• Corrigida a inicialização do Google TTS em sistemas Windows nos quais os links aceitos pelo servidor interno do navegador herdavam o modo de socket não bloqueante, causando o erro 10035 e impedindo as vozes baixadas de falar.
• O Sonarpad aguarda agora que o mecanismo WASM do Chrome ou Edge esteja totalmente carregado antes da prévia da voz ou da leitura com F5, evitando o erro “Chrome WASM TTS engine was not loaded”.
• O navegador oculto desativa a tradução de páginas e a acessibilidade do processo de renderização, evitando anúncios como “Traduzir página” e interferências com os comandos de leitura.
• O painel «Vozes no editor» mostra agora o botão «Gerenciar vozes Google...» quando o mecanismo Google está selecionado e atualiza imediatamente a lista de vozes instaladas ao fechar o gerenciador.
• Os avisos de dependências apresentados ao remover pacotes de voz Google estão agora traduzidos em todos os idiomas da interface.

Experiência de atualização
• Após uma atualização automática, a janela de conclusão com o registro de alterações abre depois da restauração inicial do foco e permanece em primeiro plano, em vez de aparecer somente depois de pressionar Tab.

Documentos PDF
• Corrigidos os PDF cujo texto incorporado continha caracteres NUL e era truncado na primeira ocorrência ao ser carregado no editor.
• Quando o pdf-extract devolve caracteres NUL incorporados, o Sonarpad tenta novamente com PDFium; quaisquer NUL restantes são removidos antes de enviar o texto aos controles do Windows, preservando o resto do documento.

Acessibilidade dos menus
• Foi removido o cálculo de mnemônicas durante a execução: as teclas de acesso estão agora escritas explicitamente em cada uma das traduções da interface e permanecem iguais em todas as inicializações.
• Foram revistas todas as entradas estáveis dos menus principais e submenus, incluindo Reprodução, fontes, Salvar imagem e Mostrar índice EPUB; mnemônicas ausentes ou duplicadas entre itens do mesmo nível foram corrigidas diretamente nas traduções.
• Os testes automáticos passam apenas a validar as traduções e falham se uma mnemônica estiver ausente, for inválida ou estiver duplicada; não alteram mais os rótulos durante a execução.
• Em menus excepcionalmente extensos, quando o texto traduzido não fornece caracteres distintos suficientes, é apresentada uma tecla de acesso numérica explícita no formato padrão do Windows «(&1)».

Versão 0.8.0 – 2026-07-15

Dicionário online
• Adicionado o alemão ao dicionário online Wiktionary.
• As configurações e os sinônimos em alemão agora são reconhecidos corretamente de acordo com a estrutura específica do Wiktionary alemão.

Confiabilidade dos audiolivros SAPI5
• A criação de audiolivros SAPI5 continua usando até 12 workers em paralelo quando a voz selecionada produz resultados confiáveis.
• Cada parte é verificada pelo tamanho do arquivo, duração estimada e uma comparação prudente com o texto atribuído.
• Partes ausentes ou suspeitas são geradas novamente com redução progressiva da concorrência: 12, 8, 6, 4, 2 e por fim 1 worker. Somente as partes problemáticas são repetidas.
• O limite confiável é lembrado separadamente para cada voz SAPI5, sem desacelerar as vozes que funcionam corretamente com 12 workers.
• Uma verificação final impede que um MP3 muito mais curto que as partes geradas seja aceito silenciosamente.
• Os detalhes são gravados em `sapi5_audiobook_diagnostic.log`.
• Cada unidade de síntese SAPI5 agora é executada em um processo Sonarpad separado e invisível. Se uma voz de terceiros falhar, apenas esse worker é encerrado e o aplicativo principal permanece aberto.
• Durante a mesma criação do audiolivro, as partes não concluídas são imediatamente repetidas com o nível de concorrência inferior seguinte; as partes já validadas são preservadas.
• A recuperação na inicialização seguinte permanece como proteção adicional apenas se o aplicativo principal ou o computador forem interrompidos.

Processos de audiolivros SAPI4
• O número de processos SAPI4 escolhido pelo usuário agora é respeitado até o máximo técnico de 64; o limite oculto anterior de 16 foi removido.
• O número efetivo só é reduzido quando o audiolivro contém menos unidades de trabalho do que o solicitado.
• Se um ou mais processos da ponte SAPI4 falharem, as partes concluídas são preservadas e apenas as unidades com falha são repetidas automaticamente com concorrência progressivamente menor.
• O Sonarpad veriagora fica o código de saída da ponte SAPI4 e rejeita partes de áudio vazias ou inválidas.

Configuração do proxy
• Foi adicionado um campo separado para a porta do proxy nas configurações de rede.
• A porta pode ser indicada independentemente do endereço, é validada entre 1 e 65535 e substitui corretamente uma porta já presente no URL.

Pesquisa de rádio por idioma e país
• Os filtros Idioma e País agora são atualizados com todas as opções disponíveis no catálogo Radio Browser e deixam de estar limitados a uma lista fixa.
• Os nomes dos idiomas agora são reconhecidos mesmo quando o Radio Browser os fornece em outro alfabeto, na forma nativa, como abreviaturas ou como combinações de vários idiomas, sendo apresentados traduzidos para o idioma atual da interface. Os valores que não representam idiomas reais, como números, gêneros musicais, países ou descrições genéricas, são filtrados.
• O catálogo é atualizado em segundo plano e mantém uma lista alternativa utilizável quando o Radio Browser não está acessível.
• As entradas de idioma do Radio Browser que se tornam idênticas após a tradução agora são reunidas em um único item da lista, evitando passos silenciosos com leitores de tela.

Melhoria principal: sincronização entre a leitura e o cursor
• A sincronização entre a leitura por voz e o movimento do cursor foi significativamente melhorada para todos os mecanismos de voz suportados.
• Quando a opção “Mover cursor durante a leitura” está ativa, o Sonarpad utiliza agora um sistema de progresso comum para Microsoft Edge Neural, Google TTS, SAPI4, SAPI5 e OneCore.
• O cursor acompanha com maior precisão o texto efetivamente pronunciado, com uma divisão mais coerente das frases e dos seus segmentos.
• Foram bastante reduzidos os avanços, atrasos, saltos irregulares e diferenças entre mecanismos de voz.
• A posição correta é melhor preservada depois de pausar, retomar, pesquisar no documento ou mudar de mecanismo.

Gravação de podcast em arquivos separados
• Adicionada a opção “Salvar o microfone e o áudio do sistema ou dos aplicativos em arquivos separados”.
• Ao gravar simultaneamente o microfone e outra fonte, o Sonarpad pode criar um arquivo somente com o microfone e outro com o áudio do sistema, de um aplicativo ou dos aplicativos selecionados.
• A separação está disponível em MP3 e WAV.
• Se a opção estiver desativada, continua sendo criado um único arquivo misturado.
• Os arquivos separados facilitam o ajuste de volume, a remoção de ruído e a edição posterior de podcasts, entrevistas e tutoriais.

Gravações de rádio programadas
• As gravações de rádio podem agora ser programadas antecipadamente.
• É possível escolher a estação, o dia, a hora e os minutos de início e a duração.
• Está disponível uma duração personalizada de 1 a 1.440 minutos.
• A gravação pode ser executada uma vez, diariamente ou semanalmente.
• A janela mostra mais claramente as gravações ativas e programadas, a data e hora previstas, a duração e o tempo restante.
• O Agendador de Tarefas do Windows pode iniciar automaticamente a gravação mesmo quando o Sonarpad não está aberto.

Calendário
• Adicionado um calendário completo e acessível por teclado.
• Permite consultar dias anteriores e seguintes, voltar rapidamente a hoje e conhecer feriados e efemérides.
• Adicionados o santo e a citação do dia, que podem ser lidos, ouvidos ou copiados.
• Os lembretes podem ser criados, alterados, excluídos, adiados e marcados como concluídos.
• Os avisos podem surgir à hora exata ou antecipadamente e utilizar o agendamento do Windows mesmo com o Sonarpad fechado.

Clima
• Adicionada uma seção de previsão meteorológica.
• É possível pesquisar uma cidade e voltar rapidamente aos locais consultados recentemente.
• Estão disponíveis as condições atuais, temperatura, valores mínimo e máximo, umidade, probabilidade de precipitação e previsão para os dias seguintes.
• É possível escolher Celsius, Fahrenheit ou seleção automática.

Filmes no cinema
• Adicionada uma seção para filmes atualmente em exibição e próximas estreias.
• Estão disponíveis a pesquisa por título, sinopse, data de estreia e reprodução do trailer.

Síntese de voz Google
• Integrado o Google TTS para leitura de documentos e criação de audiolivros.
• Adicionado um gerenciador para mostrar as vozes, filtrá-las por idioma, baixá-las e remover as que não são mais necessárias.
• É possível ajustar velocidade, volume e tom.
• O tom das vozes Google Natural é aplicado diretamente pelo mecanismo para um resultado mais natural e estável.
• Melhoradas a rapidez e a confiabilidade do Google TTS, adaptando os limites de síntese à velocidade escolhida.
• Reduzidos os tempos de espera desnecessários e melhorado o gerenciamento de erros e interrupções.

Índice de documentos EPUB
• O Sonarpad reconhece agora o índice incorporado nos livros EPUB.
• Sua presença é anunciada, e o índice pode ser aberto pelo menu Ver.
• Os capítulos e subcapítulos são apresentados hierarquicamente.
• Pressionar Enter leva imediatamente ao local selecionado.

Notícias e fontes RSS
• A seção Notícias foi ampliada com novas ferramentas de pesquisa e organização.
• Adicionada a escolha do idioma das notícias.
• É possível pesquisar nas fontes RSS e consultar notícias da própria cidade.
• As fontes da comunidade podem ser exploradas, adicionadas à coleção pessoal e enviadas à comunidade Sonarpad.

Gravação de podcast
• É possível gravar apenas o microfone, todo o áudio do sistema, um aplicativo, vários aplicativos selecionados, ou o microfone e os aplicativos ao mesmo tempo.
• Podem ser escolhidos o dispositivo e a fonte, os volumes ajustados separadamente e os níveis acompanhados em tempo real.
• Adicionadas pausa e retomada, saída MP3 ou WAV, seleção do taxa de bits MP3 e da pasta de destino.
• O computador pode ser mantido ativo durante a gravação.

Rádio
• A seção Rádio foi profundamente reorganizada.
• As estações podem ser pesquisadas por nome ou texto livre, idioma, país, cidade, gênero musical ou categoria.
• Melhorado o gerenciamento de favoritos e adicionado o restabelecimento rápido de todos os filtros.
• As estações podem ser enviadas à comunidade Sonarpad.
• Adicionadas a gravação ao vivo, a modalidade “Gravar e reproduzir”, a lista de gravações e o respectivo gerenciamento e exclusão.
• As gravações de rádio são salvas em uma pasta própria dentro da pasta geral de gravações.

Reprodução multimídia
• A estabilidade do leitor multimídia foi significativamente melhorada.
• Corrigido um problema que podia bloquear o mpv e melhorada a comunicação com o leitor.
• Melhorada a abertura de diferentes tipos de arquivos multimídia.
• O Sonarpad memoriza agora o volume utilizado durante a reprodução.
• Melhorado o gerenciamento de transmissões e gravações.
• Corrigida a abertura de arquivos a partir do Windows por duplo clique ou “Abrir com”.

Documentos PDF
• Adicionado o reconhecimento de campos de formulário em PDF.
• O Sonarpad pode encontrar campos preenchíveis, apresentá-los em uma forma textual acessível, permitir a sua edição e salvar os dados no PDF.
• Corrigido o cálculo da posição do cursor durante a leitura, sobretudo com caracteres multibyte e estruturas complexas.

Acessibilidade e teclado
• Melhorado o funcionamento dos comandos normais de edição em todo o programa.
• Copiar, cortar, colar, selecionar tudo, anular e refazer são corretamente enviados para o campo com foco, incluindo janelas auxiliares e caixas de diálogo.
• Corrigido um problema de atualização das linhas Braille.
• Melhorado o gerenciamento do foco e corrigida a escolha do idioma na Wikipédia.
• Adicionada a possibilidade de agrupar por categoria as funções do menu Ferramentas.
• Adicionadas ações configuráveis para abrir rapidamente Calendário, Clima e Filmes no cinema.

Audiolivros
• Melhorada a criação de audiolivros quando existem caixas de diálogo ou janelas modais abertas.
• O gerenciamento do progresso é mais robusto e ignora atualizações de áudio antigas.
• O Google TTS também pode ser utilizado para criar audiolivros com controle de velocidade, volume e tom.

Inteligência artificial
• O modelo Gemini padrão foi atualizado para `gemini-3.5-flash`.

Correções gerais
• Corrigidos vários bloqueios durante a reprodução com mpv.
• Corrigida a abertura de alguns arquivos de áudio e vídeo.
• Melhorado o gerenciamento dos comandos enviados para o leitor.
• Corrigida a restauração do cursor durante a leitura.
• Melhorada a estabilidade da criação de audiolivros.
• Melhorado o gerenciamento geral de multimídia, RSS, rádio e EPUB.

Versão 0.7.1 – 2026-05-13

Novidades e melhorias
• Criado o site oficial sonarpad.com, um novo ponto de referência para acompanhar as últimas novidades, baixar a versão mais recente do programa, ler os comentários dos visitantes e, no futuro, ouvir também todos os podcasts do Sonarpad. No menu Ajuda também foi adicionada a opção “Visitar sonarpad.com”, para abrir rapidamente o site oficial.
• Corrigido o problema em que arquivos com acentos ou caracteres especiais causavam erro ao iniciar a transcrição de voz.
• Agora, no menu Ver, opções como Quebra automática de linha e Mostrar vídeo durante a reprodução irão aparecer sempre com o estado correto, ativadas ou desativadas.
• Melhorada a pesquisa no YouTube, permitindo voltar com Esc à página ou ao tela anterior.
• Adicionada uma verificação prévia para confirmar se um vídeo pode ser reproduzido. A reprodução também foi melhorada: o Sonarpad agora consegue reproduzir vídeos ou playlists marcados como mix, que antes não eram reproduzidos.
• Melhorado o gerenciamento dos marcadores automáticos. Antes, se a opção Marcadores automáticos estivesse ativa e fosse depois desativada, esses marcadores permaneciam; agora o programa os ignora corretamente até que a opção seja reativada. Além disso, ao chegar no final de um arquivo multimídia, o marcador é apagado automaticamente.
• Melhorado o gerenciamento das tags com os diálogos ativos. O Sonarpad agora gerencia corretamente ambas as funções, permitindo inserir tags mesmo quando a opção de diálogos está ativa.
• Melhoradas as configurações de voz, separando claramente cada mecanismo para tornar os ajustes mais precisos. Os perfis de voz conservam corretamente as configurações de cada mecanismo individual: Edge, Sapi5 e Sapi4.
• Adicionada uma tag para inserir pausas, diretamente nas opções ou do painel de vozes pressionando Tab no editor. As opções são: 250 ms, 500 ms, 1 segundo, 2 segundos ou duração personalizada.
• Corrigido o comportamento ao reproduzir um vídeo do YouTube e iniciar a transcrição. Agora, ao voltar com Alt+Tab, o foco voltará corretamente para o botão Cancelar da transcrição em andamento.
• As transcrições agora são salvas automaticamente no final do processo.
• Melhorada a importação da Wikipédia. É possível escolher ler somente uma seção e depois, a partir do artigo, pressionar Esc para voltar à pesquisa, ou importar o artigo completo. Também é possível escolher o idioma da Wikipédia a consultar.
• Adicionada uma seção de rádios de todo o mundo, onde será possível pesquisar uma rádio por país, idioma e gênero. Também será possível adicionar rádios locais à base de dados do Sonarpad, para que outros usuários possam ouvi-las. Também é possível adicionar uma rádio aos favoritos.
• Adicionada uma seção de rotas para calcular rotas escolhendo o meio: a pé, de bicicleta, de carro ou em cadeira de rodas. É possível escolher se o rota deve ser mais curto ou mais rápido e se devem ser mostrados os municípios atravessados. Depois de importar o rota, também será possível salvar o mapa visual no menu Arquivo, Salvar imagem.
• Adicionada a opção Imprimir ao menu Arquivo. O Sonarpad imprimirá arquivos TXT usando o próprio programa e usará o programa associado para outros arquivos, como DOCX, PDF e semelhantes, para preservar ao máximo a formatação original.
• Integrado no Sonarpad um serviço de tradução para cada documento, acessível a partir do menu contextual do editor. O usuário poderá usar gratuitamente DeepL e Google Translate sem inserir nenhuma chave API; inserindo uma chave API Gemini, poderá traduzir usando Gemini.
• No menu de tradução, o usuário poderá escolher o idioma de destino. O menu reorganiza-se automaticamente: se um usuário escolher primeiro inglês, depois francês e depois italiano, estas três opções ficarão no topo do menu de idiomas.
• Se o usuário inserir a sua chave API Gemini, poderá também acessar a função Resumir texto, sempre disponível no menu contextual, para resumir qualquer artigo.
• Adicionado ao menu Reproduzir, visível durante a reprodução de um arquivo multimídia, um menu para dividir a mídia atual. Funciona com MP3, MP4 e outros formatos, dividindo por número de partes ou pela duração de cada parte.

Versão 0.7.0 – 2026-04-25

Novidades
• Adicionado suporte ao player mpv para reprodução em streaming. Vídeos do YouTube e de sites compatíveis agora são reproduzidos instantaneamente; se o usuário optar por salvá-los, eles são baixados como antes. Ao transcrever conteúdo em streaming, ele é primeiro baixado e depois transcrito. O player mpv também é utilizado para abrir vídeos locais e gerenciar legendas, garantindo maior compatibilidade com vários formatos que antes não eram bem suportados.
• Melhorada a gravação de podcasts do áudio do sistema: agora é possível escolher entre gravar todo o áudio do sistema, um único aplicativo ou vários aplicativos ao mesmo tempo. Esta opção está integrada com a gravação normal, portanto continua sendo possível ativar ou desativar o microfone separadamente.
• Adicionado o idioma hindi. Interface traduzida e adicionados RSS, changelog e guia do Sonarpad.
• Adicionada uma opção na aba Editor para mover sempre o cursor para o início da linha ao usar as setas para cima e para baixo.
• Adicionada uma opção no menu "Converter áudio" para converter áudio em M4B.

Correções
• Nos comentários do YouTube abertos usando "Reproduzir áudio por streaming...", o Sonarpad passa a carregar inicialmente apenas os primeiros 50 comentários principais, incluindo sempre todas as respostas desses comentários, e adiciona no final uma opção para carregar todos os comentários quando solicitado.
• Os marcadores agora são mostrados e geridos por ordem de posição, tanto em documentos de texto como em arquivos multimídia, em vez de seguirem a ordem de criação. Se já existir um marcador na mesma posição, ele deixa de ser adicionado novamente.
• Foi adicionada uma opção no menu Marcadores que, quando ativada, permite o gerenciamento automático dos marcadores. Ao reproduzir um arquivo local ou em streaming e fechá-lo, o Sonarpad define automaticamente um marcador com base na posição alcançada e, ao reabrir o arquivo, retoma desse ponto. O mesmo acontece com os arquivos de texto: se um texto for aberto e o cursor for movido, o Sonarpad lembrará essa posição ao fechar; se a leitura for iniciada, será salva a última frase lida e a leitura continuará exatamente desse ponto.
• Foi adicionada ao menu Ver uma opção para mostrar a renderização de vídeo para arquivos locais ou em streaming. O conteúdo de vídeo é apresentado em uma janela ampliada, onde todos os comandos ficam ocultos, exceto quando se prime a tecla Alt ou se move o mouse para a parte superior da janela. Desta forma, os usuários com baixa visão deverão ter um conteúdo maior e mais fácil de utilizar.

Versão 0.6.9 – 2026-04-08

Correções
• A experiência de Localizar nos arquivos foi melhorada: ao abrir Pesquisar pasta, o foco vai diretamente para a lista de pastas; ao abrir um resultado com Enter, todos os comandos de teclado continuam a funcionar; ao pressionar Esc, retorna ao resultado anteriormente selecionado; e ao voltar com Alt+Tab, o foco vai para o campo de pesquisa ou para a lista de resultados, se está estiver aberta.
• O F5 iniciava sempre a leitura desde o início. Isso foi corrigido e a leitura passa a começar na posição atual do cursor, preservando também `Shift+F5` e `Ctrl+F5` para ir para a frase anterior ou seguinte.
• Depois de usar Ir para a linha, ao pressionar Esc o foco podia sair do Sonarpad. Agora volta corretamente ao editor.
• A opção `Quebra automática de linha` agora é aplicada imediatamente também aos documentos já abertos, sem precisar reabrir o arquivo.

Versão 0.6.8 – 2026-04-07

Novidades
• Adicionado um novo item no menu Reproduzir que permite transcrever qualquer arquivo de áudio ou vídeo com o Whisper. Nas Opções existe uma nova seção chamada «IA e Transcrição», onde é possível escolher o modelo, ativar o suporte opcional a CUDA para placas gráficas NVIDIA, manter o idioma original e ativar ou desativar as marcas temporais.
• Foi adicionada ao menu Reproduzir a nova ação `Transcrever pasta atual`, que transcreve todos os arquivos de áudio suportados da pasta da mídia aberta e reúne tudo em um único documento, com janela de progresso dedicada, indicação do arquivo atual e possibilidade de cancelar. Também pode ser iniciada com `Alt+Shift+C`.
• Adicionada a possibilidade de usar ditado por voz offline, com o mesmo funcionamento da transcrição de áudio. Por padrão, pressione `Ctrl+Shift+Espaço` para iniciar o ditado e pressione o mesmo atalho novamente para encerrá-lo; o atalho pode ser personalizado nas Opções. A partir da segunda ativação, o ditado torna-se mais rápido porque o mecanismo permanece pronto na memória; em PCs com menos de 4 GB de RAM, este carregamento antecipado e reutilização são desativados automaticamente.
• Foi adicionada nas Opções do editor uma nova configuração, desativada por padrão, que faz com que `Esc` feche a janela do editor.
• A pesquisa de podcasts passa a usar `iTunes + Spreaker` por padrão, com filtragem de resultados duplicados quando o mesmo podcast está presente em ambas as plataformas.
• Melhorada a pesquisa e navegação de podcasts Apple: a pesquisa de podcasts, a navegação por categorias e os top podcasts por categoria passam a usar o país selecionado para o pasta de podcasts. Em Opções > RSS / Podcast, pode deixar em `Automático` para usar o país do sistema ou escolher manualmente outro país.
• O limite de resultados para as categorias de podcasts Apple foi aumentado. Na primeira abertura continuam a ser carregados apenas os primeiros 50 resultados como antes; ao escolher `Carregar mais resultados`, o Sonarpad carrega até 200 resultados no total (limite imposto pela Apple) e permite navegar pelas páginas seguintes mantendo uma experiência fluida.
• O Sonarpad está agora disponível também no Mac, embora com um conjunto de recursos parcial. Link do projeto: https://github.com/Ambro86/Sonarpad-Mac

Melhorias
• Foram adicionados mais de 50 países selecionáveis para o pasta de podcasts, permitindo escolher entre muitos mais catálogos nacionais.
• "Reproduzir áudio por streaming..." agora também permite pesquisar no YouTube escrevendo qualquer texto ou colar o link de um canal ou de uma playlist do YouTube para mostrar os respectivos resultados.
• A apresentação dos resultados em "Reproduzir áudio por streaming..." foi melhorada: as entradas do YouTube agora incluem título, duração, canal e visualizações em um formato mais claro.
• "Reproduzir áudio por streaming..." passa a suportar também os comentários do YouTube: podem ser abertos a partir do menu contextual, é possível ler as respostas e expandir os tópicos de comentários com a Seta para a direita.
• Foram adicionados favoritos do YouTube para canais e playlists em "Reproduzir áudio por streaming...": podem ser adicionados a partir dos resultados através do menu contextual, abertos diretamente a partir da lista Favoritos acessível com Tab logo após o campo de URL/pesquisa do YouTube e removidos mais tarde dessa mesma lista também pelo menu contextual. Nos resultados de pesquisa do YouTube, o menu contextual está disponível somente para canais e playlists.
• "Reproduzir áudio por streaming..." agora pode pedir credenciais quando um site exige início de sessão. O usuário pode inseri-las, guardá-las para esse site e gerenciar depois as credenciais salvas em Opções > Áudio.
• Melhorado o foco durante "Reproduzir áudio por streaming...", para que a janela de progresso permaneça mais estável durante a descarga e a conversão.
• Adicionadas duas novas ações de leitura no menu Voz: `Frase anterior` e `Próxima frase`, com atalhos configuráveis para saltar durante a leitura do texto.
• O atalho padrão de `Executar arquivo com interpretador` agora é `Ctrl+Shift+F5`, para que `Shift+F5` possa ser usado por padrão para `Frase anterior`.
• Adicionado o gerenciamento de perfis de voz em Opções > Voz: é possível adicionar, renomear e excluir perfis.
• Ampliadas em Opções > Áudio as opções do intervalo de retrocesso durante a reprodução, com novos valores de 1 segundo até 2 horas.
• Adicionada a tradução russa graças a Dmitriy.
• Adicionada em Opções > Áudio uma nova opção para escolher o formato do nome das partes do audiolivro: `Título + número`, `Somente número` ou `Número + título`.
• Adicionada no menu de contexto dos artigos RSS a ação para adicionar o artigo aos favoritos.
• A fonte RSS "Favoritos" pode ser excluída e é recriada automaticamente quando um novo artigo é adicionado aos favoritos.
• Adicionados atalhos de teclado RSS para mover as fontes para cima/para baixo: `Ctrl+Shift+Seta para cima` e `Ctrl+Shift+Seta para baixo`.
• Melhorada a janela RSS com uma prévia integrada do artigo, permitindo consultar o texto diretamente ali e alcançá-lo rapidamente com Tab antes de abrir o artigo completo no editor.
• Adicionada no RSS uma entrada explícita «Carregar mais notícias» no final das fontes quando existem mais itens disponíveis; ao pressionar Enter é carregado o bloco seguinte e o foco passa para o primeiro artigo novo.
• No dicionário de voz, ao adicionar ou editar uma substituição, existe agora uma caixa «Distinguir maiúsculas e minúsculas» para decidir se cada substituição deve respeitar ou ignorar a capitalização.
Correções
• "Reproduzir áudio por streaming..." passa a respeitar o limite de cache de podcasts já definido nas Opções, e esse mesmo limite também se aplica à reprodução de audiodescrições.
• Corrigida a importação a partir da Wikipédia, que em algumas páginas não importava corretamente as citações presentes no texto.
• Melhorado o parser de páginas web: em algumas páginas WordPress não eram incluídos os itens de listas nem alguns títulos de seção.
• Agora, ao usar «Ir para a linha», o campo é pré-preenchido com a linha atual.
• Corrigida a exportação OPML de podcasts e feeds RSS, que agora gera arquivos aceitas pelo iTunes.
• Corrigida a transcrição de arquivos multimídia: agora, ao fechar com Alt+F4 o documento gerado, o Sonarpad pergunta se pretende salvar o arquivo e propõe o nome correto com base no nome do arquivo transcrito, em vez da primeira linha do texto.
• Adicionadas mensagens de confirmação localizadas para a correta importação e exportação OPML de fontes RSS e podcasts.
• Foi corrigido um problema em que, em "Reproduzir áudio por streaming...", ao digitar um texto de pesquisa e selecionar um canal do YouTube nos resultados, o programa podia parecer bloqueado em vez de abrir os vídeos desse canal.
• Corrigido um erro em que a lista de arquivos abertos era mostrada no menu Ajuda em vez do menu Janela.
• Corrigido um caso limite no streaming em que a reprodução podia iniciar, mas a janela “Transferência de streaming” permanecia aberta quando o arquivo baixado já correspondia ao formato de destino.
• Corrigido o comportamento de conversão no streaming MP3: quando o stream já está em MP3 e o usuário escolhe um taxa de bits MP3 explícito (por exemplo 128 kbps), o Sonarpad agora recodifica para o taxa de bits selecionado em vez de saltar a conversão.
• Corrigido o atalho `Alt+Shift+L`: agora abre corretamente a lista de capítulos durante a reprodução.
• Corrigido o atalho `Alt+Shift+T`: agora inicia corretamente «Transcrever áudio atual» em vez de abrir o menu Ferramentas.
• Se já estiver a ser reproduzido um áudio, ao iniciar a transcrição o Sonarpad coloca esse áudio automaticamente em pausa antes de começar.
• Corrigido um problema em que, ao importar um artigo da Wikipédia, a importação podia ter êxito mas o texto do artigo não era mostrado no tela.
• Adicionado suporte a capítulos de podcast incorporados em arquivos multimídia locais (por exemplo, metadados de capítulos MP3): quando o feed/URL não fornece capítulos, o Sonarpad passa a carregá-los do arquivo baixado em segundo plano, permitindo início imediato da reprodução e aplicação dos capítulos assim que tornam-se disponíveis.
• Corrigido o carregamento de capítulos para episódios de podcast baixados e abertos como arquivos multimídia locais normais: os capítulos incorporados passam a estar disponíveis também nesse caso, e não apenas quando a reprodução começa na janela Podcasts.
• Corrigida a finalização dos audiolivros MP3 com SAPI4 e SAPI5: o arquivo final agora é finalizado corretamente, evitando arquivos incompletos ou frágeis após exportações longas.
• Adicionada uma barra de progresso explícita para a fase de finalização em todos os modos de criação de audiolivros: após a criação, o Sonarpad anuncia e mostra a finalização com progresso visível.
• Corrigido um erro nas vozes de diálogo: os parâmetros de velocidade/tom/volume da primeira e da segunda voz de diálogo agora são aplicados corretamente durante a síntese.
• Melhorada a detecção de codificação para arquivos japoneses `.txt`: adicionado fallback seguro Shift_JIS/CP932 em casos de mojibake, preservando o comportamento existente para UTF/diacríticos/chinês.
• Refatoração interna de segurança: conversão para implementações safe sempre que possível e redução drástica das linhas de código unsafe.

Versão 0.6.7 – 2026-03-02
Melhorias
• Ora il programma riesce a gestire Sostituisci tutto in modo massivo su file grandi con un gran numero di sostituzioni.
• Tradução polaca atualizada graças ao DJ Graco.
• Adicionada a tradução lituana.
• Adicionada a tradução chinesa.
• Agora, compilações beta frequentes serão publicadas na seção Releases do projeto, para que os usuários possam testar as novas alterações antes da próxima versão estável.
• Adicionado o atalho `Ctrl+.` para inserir o caractere de reticências (…).
• Melhorado o suporte a capítulos de podcast: a navegação por capítulos está agora mais confiável, incluindo episódios diretos/streaming em que os capítulos não estão incorporados no arquivo MP3, usando metadados de capítulos do feed/URL como fallback quando disponíveis. Adicionados os atalhos `Ctrl+Alt+PageUp` (capítulo anterior) e `Ctrl+Alt+PageDown` (capítulo seguinte).
• Reorganizadas as pastas de saída em `Documentos\\Sonarpad`: os arquivos agora são salvos nas subpastas dedicadas `audiobooks`, `documents`, `recordings` e `media`, com migração automática dos caminhos antigos.
• Melhorado o suporte para arquivos de texto muito grandes (incluindo 60 MB): abertura e navegação linha a linha mais fluídas, especialmente com leitores de tela.
• Guias atualizados para todos os idiomas e recursos de localização atualizados em toda a app, incluindo textos de doações e traduções do instalador NSIS (novas strings em chinês simplificado e lituano, além da conclusão da tradução ucraniana do setup).
• Adicionado suporte global de proxy de rede (HTTP/HTTPS e SOCKS5/SOCKS5H) para recursos online, com validação ao salvar Opções: proxies inválidos são avisados e removidos automaticamente.
• Adicionada uma nova função em Ferramentas: "Reproduzir áudio por streaming...", que permite colar um URL (YouTube ou link multimídia direto), escolher o formato de saída e o perfil de qualidade/taxa de bits (incluindo qualidade/taxa de bits original para MP3 e MP4) e iniciar a reprodução no leitor de áudio do Sonarpad.
• Adicionado suporte à tecla multimídia de sistema Reproduzir/Pausar (fones de ouvido/teclado): agora controla tanto a reprodução multimídia como a pausa e retomada da leitura de texto (com prioridade para o leitor multimídia quando ambos estão ativos).
• Adicionada uma nova opção em Arquivo > Arquivos recentes: "Limpar arquivos recentes" para esvaziar rapidamente a lista de documentos recentes.
• Ampliadas as opções de taxa de bits na conversão de áudio e na gravação de podcast: adicionados valores mais baixos (64/96 kbps) e MP3 estendido até 320 kbps, com validação e tratamento do encoder alinhados.
• Ampliadas as opções de divisão de audiolivros por tempo até 60 minutos.
• Melhorada a divisão de audiolivros por partes: agora o número de partes pode ser inserido manualmente, com validação de 1 a 100.
• Adicionado o novo modo Ver > Somente leitura para bloquear edições acidentais no texto, mantendo leitura e navegação completas dos documentos.
• Adicionada uma barra de progresso acessível durante as atualizações do programa, para que leitores de tela possam acompanhar em tempo real o progresso da transferência.
• Adicionada uma nova barra de estado discreta na janela principal com contagem de caracteres, palavras e posição linha/coluna (por exemplo: "Caracteres (com espaços): 11. | Palavras: 2. | Ln 1, Col 12"), sem interferir com o foco do NVDA.
• Adicionada uma nova opção no menu Ver para quebra automática de linha, permitindo ativar/desativar rapidamente sem abrir as Opções.
• Adicionadas em Editar > Texto novas ações para aumentar/reduzir recuo, com atalhos Ctrl+Shift+. (indentar) e Ctrl+Shift+, (desindentar), porque quando “Mostrar vozes no editor” está ativo a tecla Tab fica reservada para a navegação do painel de vozes.
• Adicionada a exibição localizada de data e hora em artigos RSS e episódios de podcast, com formato adaptado ao idioma da interface.
• Adicionada no menu de contexto RSS uma nova ação para compartilhar por e-mail o artigo selecionado.
• Adicionadas opções granulares de confirmação de remoção em Opções > RSS e podcast: para RSS (feed/artigo/ambos/nenhum) e para Podcasts (podcast/episódio/ambos/nenhum).
• Adicionada cópia rápida de RSS configurável com Ctrl+C (Opções > RSS e podcast): copiar título, URL, conteúdo do artigo ou tudo junto.
• Fluxo de RSS unificado: “Adicionar fonte” agora aceita tanto URL de feed quanto palavras-chave (com geração automática do feed do Google News), sem necessidade de pesquisa separada.
• Ao pressionar Ctrl+A, o programa agora anuncia a conclusão da ação para um feedback mais claro em leitores de tela.
• Adicionado Shift+F3 para "Localizar anterior" no menu Editar, em complemento ao F3 "Localizar seguinte".
• Melhorada a mensagem de substituição com singular/plural corretos (por exemplo, “1 substituição realizada” vs “2 substituições realizadas”).
• Adicionada na janela do dicionário a seleção de idioma de pesquisa, com Auto (idioma da interface) por padrão e possibilidade de escolha manual.
• Adicionada uma nova aba de Atalhos nas Opções para personalizar combinações de teclas, com detecção de conflitos e aviso quando um atalho já está atribuído a outra ação.
• Adicionado suporte inicial a parâmetros de linha de comandos: `-h`/`--help` mostram a ajuda rápida e `--version` mostra a versão do programa.
• Melhorada a clareza do ajuste manual de velocidade e tom: os campos manuais agora usam uma escala centrada em 100, onde 100 corresponde ao valor normal.
• Melhorada a seleção de vozes Microsoft em Opções > Voz e no painel de vozes do editor: foi adicionada uma caixa de combinação de idioma localizada para filtrar vozes por idioma, mantendo o modo “somente vozes multilíngues” como lista única sem divisão por idioma (com a caixa de combinação de idioma oculta quando ativa).
• Adicionada a configuração de voz para diálogos em Opções > Voz com navegação completa por Tab, usando o mesmo modelo de vozes da interface principal (mecanismo, filtro de idioma Edge, voz e velocidade/tom/volume com rótulos); adicionada também uma segunda voz de diálogos opcional com os mesmos controles (mecanismo, filtro de idioma Edge, voz, velocidade/tom/volume) para alternar diálogos; as regras de diálogos são salvas na configuração `.ini`, sem modificar o texto do documento.
• Melhorada a rótulo de Desfazer: a opção Editar > Desfazer agora mostra qual ação será desfeita (por exemplo, edição de texto, comentar/descomentar linhas ou inserção de tag de voz), mantendo-se indisponível quando não há nada para desfazer.
Correções de bugs
• Corrigido o suporte de abertura RTF: os arquivos `.rtf` agora são extraídos e mostrados como texto legível, em vez de markup RTF bruto (ex.: `{\\rtf1...}`).
• Corrigida a abertura de arquivos de texto chineses em codificação GB18030/GBK: o Sonarpad agora detecta e decodifica corretamente, evitando texto ilegível (mojibake).
• Melhorada a criação de audiolivros M4B com metadados e marcadores de capítulos; corrigido o problema "chipmunk" (voz demasiado aguda/rápida) nos arquivos M4B gerados.
• Corrigida a interface de taxa de bits na janela de gravação de audiolivro: removidos textos fixos em italiano e adicionada a opção 64 kbps entre os taxa de bitss selecionáveis.
• Corrigido "Salvar tudo" (Ctrl+Shift+S): agora todos os documentos abertos modificados são detectados de forma confiável (incluindo abas novas/sem salvar) e o Salvar tudo grava cada um corretamente, abrindo "Salvar como" quando necessário.
• Corrigida a ordenação dos artigos RSS do Google News: quando a data está disponível, os artigos agora são mostrados do mais recente para o mais antigo.
• Corrigida a associação de rótulos no NVDA na janela do dicionário: o campo de pesquisa e a lista de idioma agora anunciam a rótulo correta.
• Corrigida a navegação por teclado na janela Propriedades de RSS/Podcast: Tab/Shift+Tab agora alcançam o botão OK, Enter ativa o OK, Esc fecha com segurança e o foco volta corretamente à lista RSS/Podcast.
• Corrigido o histórico de desfazer em RSS/Podcast: o Ctrl+Z agora suporta desfazer em múltiplos níveis para remoções (artigos/episódios e fontes), e não apenas a última ação.
• Melhorados os anúncios de remoção em RSS/Podcast com mensagens explícitas (RSS removido, artigo RSS removido, episódio de podcast removido).
• Melhorado o comportamento de foco após remover/desfazer em RSS/Podcast: no RSS, o primeiro feed volta a ser selecionado de forma confiável quando necessário, e foram reduzidas repetições de anúncios do leitor de tela durante a re-seleção atrasada.

Versão 0.6.6 – 2026-02-13
Melhorias
• Adicionada "Formatação automática para TTS" no menu Editar para preparar rapidamente o texto para voz (remove markdown/aspas e recompõe linhas quebradas).
• Melhorada a inserção de tags de voz: quando há texto selecionado, as tags agora são aplicadas corretamente tanto em uma única linha quanto em seleção multilinha.
• Adicionada uma opção nas Configurações de áudio para escolher a pasta padrão de gravação de audiolivros (padrão: Documentos\\Sonarpad Audiobooks).
• Na janela de gravação de audiolivro, quando a divisão em partes está ativa, foi adicionada uma nova opção (ativada por padrão) para criar uma subpasta dedicada às partes geradas.
• A exportação de audiolivros agora guarda MP3 em estéreo com taxa de bits escolhido pelo usuário para vozes Edge, SAPI5 e SAPI4.
• Adicionado suporte a vozes SAPI5 de 32 bits via bridge, para usar também vozes disponíveis somente em mecanismos de 32 bits.
• Reorganizados os recursos de voz em um menu dedicado "Voz e áudio" e adicionada/esclarecida a opção "Converter áudio", útil para converter qualquer arquivo multimídia suportado para MP3, AAC, OGG, Opus, FLAC, WAV e AIFF.
• Adicionada a remoção de artigos RSS individuais e episódios de podcast individuais (tecla Delete + menu de contexto com confirmação), sem remover toda a fonte RSS/podcast, com anulação da última remoção (artigo/episódio individual ou fonte RSS/podcast completa).
• Adicionada a exportação de feeds RSS para OPML na janela RSS, para salvar e reimportar facilmente as fontes atuais.
• Adicionada a função "Pesquisar RSS por palavra-chave" na janela RSS: ao inserir uma palavra-chave, o Sonarpad gera automaticamente o URL RSS do Google News e abre a janela de adicionar fonte já pré-preenchida, permitindo criar um feed temático em um único passo.
• Adicionada a tradução sérvia graças a Mila Kuran.
• Adicionada a tradução ucraniana graças a Ivan Shtefuriak.
• Adicionada a abertura múltipla de arquivos multimídia: ao abrir vários arquivos de uma vez é criada uma fila de reprodução em vez de substituir o arquivo atual.
• Adicionados atalhos de avanço/retrocesso variável durante a reprodução: com base de 1 minuto, Esquerda/Direita avança 60s, Shift+Esquerda/Direita avança 20s e Ctrl+Esquerda/Direita avança 3 minutos.
• Adicionados atalhos de faixa anterior/seguinte no leitor: Ctrl+PageUp e Ctrl+PageDown.
• Adicionada a opção "Restaurar volume" e agrupadas as ações de restauração em um submenu dedicado "Restaurar" em Reprodução, juntamente com "Restaurar velocidade" e "Restaurar tom".
• Melhorias no instalador: o setup.exe agora permite escolher entre associar todos os tipos de arquivo suportados ou selecionar manualmente as extensões; o MSI também passa a oferecer seleção por extensão na árvore de recursos (o padrão mantém-se: tudo ativo).
• Adicionado o novo menu "Janela" com a opção "Documentos abertos..." para alternar rapidamente para qualquer arquivo atualmente aberto.
• Atualizada a opção Ver > Fonte: o seletor completo foi substituído por um submenu rápido com fontes comuns (Arial, Calibri, Consolas, Segoe UI, Tahoma, Verdana, Times New Roman, Georgia), mantendo o tamanho de texto atual.
• Melhorada a leitura de RSS e podcasts com dois avisos distintos: os nós da fonte anunciam "novos itens" quando um feed/podcast tem novidades, enquanto artigos RSS e episódios de podcast individuais anunciam "não lido"/"não reproduzido"; este comportamento pode ser desativado nas Opções.
Correções de bugs
• Corrigida a extração de texto EPUB para livros com comentários HTML inline (<!-- ... -->): o texto dos capítulos agora é analisado corretamente em vez de ser parcialmente ou totalmente ignorado.
• Corrigido o dicionário Wiktionary em espanhol e o cache do dicionário: palavras como "agua" agora são encontradas corretamente e entradas antigas de "Palavra não encontrada" não são mais reutilizadas.
• Corrigida a codificação na importação de artigos RSS para algumas fontes em espanhol (ex.: El Mundo): acentos e "ñ" agora são preservados corretamente no editor temporário.
• Corrigida a decodificação ANSI de arquivos da Europa Central (ex.: checo/polaco): o Sonarpad agora distingue melhor UTF-8 e ANSI e escolhe a code page correta (incluindo Windows-1250), evitando diacríticos corrompidos.
• Corrigida a persistência de fontes RSS com parâmetros na URL (ex.: `rss.aspx?c=...`): esses feeds agora são salvos e restaurados corretamente após reiniciar o Sonarpad.
• Corrigida a abertura de arquivos ponteiro do Google Drive (`.gdoc`, `.gsheet`, `.gslides`) a partir do menu de contexto do Explorador: quando a leitura direta falha com “Incorrect function (os error 1)”, o Sonarpad agora usa fallback por shell-open e o documento abre corretamente.
• Corrigida a leitura de arquivos Excel legacy `.xls` (Excel 2010): arquivos binários antigos agora são detectados/decodificados corretamente em vez de mostrar texto corrompido (ex.: `ÐÏ_à¡±...`).
• Corrigido o fluxo de anúncio do corretor ortográfico: os erros voltam a ser anunciados ao rever o texto mais tarde, e o mesmo erro é novamente restaurartado se for apagado e reescrito.
• Corrigidas as operações de texto por linha (ex.: Ctrl+Q / Ctrl+Shift+Q, ordenar/inverter/linhas únicas/unir linhas): ao selecionar somente uma linha com Shift+Seta para baixo, as linhas adjacentes não são mais unidas nem truncadas.
• Corrigido o comportamento em seleções multilinha nas operações por linha (Ctrl+Q / Ctrl+Shift+Q e ferramentas relacionadas): quando o RichEdit devolve abas de linha somente com CR, agora são normalizados corretamente e todas as linhas selecionadas são processadas sem cortar o primeiro caractere.
• Ampliada a normalização de entrada TTS para símbolos visíveis de espaço/tab/nova linha (␠/U+2420, ␣/U+2423, ␉/U+2409, ␊/U+240A, ␍/U+240D, ␤/U+2424), que com vozes multilíngues podiam causar repetição de parágrafos.
• Refinada a sanitização do texto para Edge TTS com uma única rotina de validação: normalização de espaços estranhos/invisíveis, compactação de sequências longas de pontuação (como "...", "!!!", "???") e descarte de trechos compostos só por pontuação para evitar loops de reprodução.
• Corrigido o anúncio do tempo de reprodução (Ctrl+I) para streams MP3/podcast: o tempo atual agora é limitado à duração da faixa, e a reprodução é interrompida automaticamente se a posição ultrapassar o fim.
• Melhorada a cobertura de localização do instalador: o setup.exe agora inclui também checo, polaco, francês e sérvio, enquanto o MSI permanece como um único pacote en-US para evitar confusão nas releases.
• Corrigida a limpeza na desinstalação das entradas do menu de contexto: "Abrir com Sonarpad" agora é removido de forma confiável, inclusive em cenários legados de registro.
• Corrigida a confiabilidade de pausar/retomar no SAPI5: a pausa com F4 agora funciona corretamente e, ao retomar, volta ao ponto esperado em vez de reiniciar do início.
• Corrigido o fluxo pausar + pesquisar + retomar na reprodução multimídia: após pausar e avançar/recuar com Esquerda/Direita, ao pressionar Espaço a reprodução é retomada de forma confiável na posição atual em vez de parar ou reiniciar do início.

Versão 0.6.5 – 2026-02-07
Melhorias
• Tradução em espanhol aprimorada graças a Arturo Fernandez Rivas.
• Adicionada uma opção para dividir audiolivros EPUB por capítulos.
• As importações RSS agora usam uma aba temporária dedicada (título localizado); Salvar como a converte em um documento normal.
• As mensagens do leitor de tela agora também são enviadas ao JAWS quando disponível.
Correções de bugs
• A leitura a partir do cursor (F5) agora começa exatamente no cursor. Antes podia começar algumas linhas acima porque o deslocamento do cursor não correspondia às posições CRLF/UTF-16.
• Corrigido um problema de redesenho: ao digitar sobre uma seleção, o texto anterior podia desaparecer até mover a seleção.
• Corrigido o parser de capítulos EPUB: páginas de capa ou somente com imagens não geram mais leitura de CSS (ex.: "padding") nem títulos "Sconosciuto".
• Corrigida a falha ao dividir por tempo audiolivros usando EPUB: o Edge TTS podia falhar com trechos vazios ou muito longos ("Edge audio not sent").
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
• Corrigida a limpeza de Markdown: agora trata marcadores '*' quando a preservação de listas está desativada.
Correções de bugs
• Corrigido um bug em que audiolivros com SAPI4 podiam ser criados de forma diferente do esperado.
• Janela Buscar em arquivos: ao pressionar Enter em um resultado agora abre na posição correta do trecho e Esc volta aos resultados.
• Janela Opções: ajustada a formatação visual das abas Geral, Voz, Editor e Áudio para evitar controles ausentes ou cortados.
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
Novos recursos
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
• Adicionada leitura sincronizada de legendas (srt, vtt, ass, sub, sbv, lrc, smi) com NVDA ou voz selecionada. O programa pesquisa um arquivo de legendas com o mesmo nome do arquivo de mídia. Adicionadas as opções "Importar legendas" e "Remover legendas" no menu Reprodução para arquivos com nomes diferentes.
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
• SAPI 4: Excluído o gargalo WAV-MP3 convertendo fragmentos em paralelo durante a síntese.
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
• Corrigido um erro que impedia a adição de URLs normais na recurso de feeds RSS.
• Corrigido um problema em que o idioma da Wikipedia era mostrado em várias abas das opções.
• Removida a criação de alguns arquivos de depuração que eram gerados mesmo em modo release.
Melhorias
• Melhorado o suporte para vozes Microsoft, que agora são reproduzidas utilizando um método dedicado com um user agent diferente.
• Adicionado suporte para arquivos MP4.

Versão 0.6.0 – 2026-01-XX
Melhorias
• Melhorado o suporte para vozes Microsoft, que agora são reproduzidas utilizando um método dedicado com um user agent diferente.

Versão 0.6.0 – 2026-01-20
Novos recursos
• Adicionado o corretor ortográfico. A partir do menu contextual, é possível verificar se a palavra atual está correta e, caso não esteja, obter sugestões.
• Adicionada a importação e exportação de podcasts por meio de arquivos OPML.
• Adicionado suporte à pesquisa no Podcast Index além do iTunes. O usuário pode introduzir a sua API key e API secret gratuitos (gerados somente com o seu endereço de e-mail).
• Adicionado suporte às vozes SAPI4, tanto para leitura em tempo real como para a criação de audiolivros
• Adicionado um fallback automático de OCR para PDFs não acessíveis: quando não é encontrado texto extraível, o documento é reconhecido por meio de OCR..
• Adicionado suporte de dicionário através do Wiktionary. Ao pressionar a tecla Aplicativos, são apresentadas as configurações e, quando disponíveis, também sinônimos e traduções para outros idiomas.
• Adicionada a importação de artigos da Wikipedia com pesquisa, seleção de resultados e importação direta para o editor.
• Adicionado o atalho Shift+Enter no módulo RSS para abrir um artigo diretamente no site original.
Melhorias
• A seleção do microfone agora é sempre respeitada pelo aplicativo.
• Na janela de podcasts, ao pressionar Enter em um episódio, o NVDA anuncia imediatamente “carregando”, fornecendo confirmação imediata da ação.
• Nos resultados de pesquisa de podcasts, ao pressionar Enter, o usuário passa a subscrever o podcast selecionado.
• Corrigidas e melhoradas as rótulos dos atalhos Ctrl+Shift+O e Podcast Ctrl+Shift+P.
• A velocidade de reprodução e o volume agora são salvos nas configurações e mantêm-se para todos os arquivos de áudio.
• Adicionada uma pasta de cache dedicada para os episódios de podcasts. O usuário pode manter os episódios por meio da opção “Manter podcast” no menu Reproduzir. O cache é limpo automaticamente quando ultrapassa o tamanho definido pelo usuário (Opções → Áudio).
• Melhorada de forma significativa a obtenção de artigos RSS utilizando libcurl com impersonação de Chrome e iPhone, garantindo compatibilidade com cerca de 99 % dos sites.
• Adicionado o estado lido / não lido para os artigos RSS, com indicação clara na lista RSS.
• A função Substituir tudo agora mostra também o número de substituições efetuadas.
• Adicionado o botão Excluir podcast ao navegar pela biblioteca de podcasts através da tecla Tab.
Correções
• Removida a entrada redundante “pending update” do menu Ajuda (as atualizações já são geridas automaticamente).
• Corrigido um erro em que, ao abrir um arquivo MP3 e pressionar Ctrl+S, o arquivo era salvo e ficava corrompido.
• Corrigido um problema de interface em que “Batch Audiobooks” era apresentado como “(B)… Ctrl+Shift+B” (removida a rótulo redundante).
• Corrigido o funcionamento das aspas inteligentes: quando ativadas, as aspas normais agora são corretamente substituídas por aspas tipográficas.
• Corrigido um erro em que, ao utilizar “Ir para o marcador”, a velocidade de reprodução era redefinida para 1.0.
• Corrigido um problema em que episódios de podcasts já baixados eram novamente baixados em vez de ser utilizada a versão em cache.
Atalhos de teclado
• F1 agora abre o guia.
• F2 agora verifica a existência de atualizações.
• F7 / F8 agora permitem navegar para o erro ortográfico anterior ou seguinte.
• F9 / F10 agora permitem alternar rapidamente entre as vozes salvas nos favoritos.
Melhorias para desenvolvedores
• Os erros deixaram de ser ignorados silenciosamente: todos os padrões let _ = foram removidos e os erros agora são tratados explicitamente (propagados, registrados ou tratados com mecanismos de alternativas adequadas).
• O projeto agora não compila se existirem avisos: tanto cargo check como cargo clippy devem ser concluídos sem avisos, com verificações mais restritivas e remoção de permissões `allow` sempre que possível.
• Removidas implementações personalizadas do tipo strlen / wcslen. Os comprimentos de strings e buffers UTF-16 agora são derivados de dados gerenciados pelo Rust, sem varrimentos manuais de memória.
• O gerenciamento das DLLs foi simplificado e consolidado em torno de libloading, evitando lógica de carregamento personalizada e análise de PE.
• Removidas as funções auxiliares manuais de parsing de bytes: todo o processamento passa a usar from_le_bytes / from_be_bytes sobre fatias verificadas.
Essas alterações reduzem o uso desnecessário de unsafe, eliminam possíveis comportamentos indefinidos e tornam a base de código mais idiomática, robusta e fácil de manter.

Versão 0.5.9 - 2026-01-13
Novos recursos
• Adicionada a possibilidade de reordenar RSS pelo menu contextual (cima/baixo/posição), com validação de posições inválidas.
• Adicionado menu contextual para artigos com opções para abrir o site original e compartilhar via WhatsApp, Facebook e X.
• Adicionado atalho Esc para voltar de artigos importados para a lista de RSS.
• Adicionada a modalidade podcast: buscar, inscrever e ouvir; reordenar assinaturas; Esc para parar a reprodução e voltar à lista; Enter em um episódio inicia a reprodução.
• Adicionado controle de velocidade de reprodução para podcasts e arquivos MP3.
• Adicionado Ctrl+T para ir a um tempo específico.
• Adicionado um botão de prévia da voz após a caixa de volume.
• Adicionada a função regex para Localizar e Substituir, estilo Notepad++.
• Adicionada a importação de RSS usando arquivos OPML e TXT.
• Adicionada nas Opções a caixa para habilitar "Abrir com Sonarpad" no Explorador de arquivos, inclusive na versão portátil.
Melhorias
• Melhorada a seleção de velocidade, tom e volume das vozes, respeitando os limites máximos do TTS.
• Várias melhorias no RSS para baixar todos os artigos sem mover o foco do NVDA durante atualizações.
• Melhorada a reprodução de áudio com um menu dedicado, anúncio de tempo com Ctrl+I e volume até 300%.
• Adicionados atalhos faltantes para algumas funções.
• Reorganizado o menu Editar com um submenu para as funções de limpeza de texto.
• Reorganizadas as Opções em abas, com Ctrl+Tab e Ctrl+Shift+Tab para navegar.
• Resolvidos os problemas de leitura de artigos: o leitor RSS agora mostra os artigos completos como no navegador.
Correções
• Corrigido um problema em que a limpeza de Markdown removia números no início da linha.
• Corrigido AltGr+Z que acionava Undo.
• Corrigido um problema em que ao gravar um audiolivro não era possível interromper rapidamente.
Localização
• Adicionada a tradução vietnamita (graças a Anh Đức Nguyễn).

Versão 0.5.8 - 2026-01-10
Novos recursos
• Adicionado controle de volume para o microfone e o áudio do sistema ao gravar podcasts.
• Adicionada uma nova função para importar artigos de sites ou feeds RSS, incluindo os feeds mais importantes para cada idioma.
• Adicionada uma função para remover todos os marcadores do arquivo atual.
• Adicionada a função para remover linhas duplicadas e linhas duplicadas consecutivas.
• Adicionada a função para fechar todas as abas ou janelas exceto a atual.
• Adicionada a entrada Doações no menu Ajuda para todos os idiomas.
Melhorias
• Melhorado o terminal acessível para evitar alguns travamentos.
• Melhoradas e corrigidas as teclas de acesso e os atalhos de teclado do programa.
• Corrigido um problema em que, ao fechar a janela de reprodução de áudio, a reprodução não parava.
• Adicionadas janelas de confirmação para ações importantes (ex.: remover linhas duplicadas, remover hifens no final da linha, remover todos os marcadores do arquivo atual). Nenhuma confirmação é mostrada se a ação não se aplica.
• Adicionada a possibilidade de excluir feeds/sites RSS da biblioteca selecionando-os e pressionando Delete.
• Adicionado um menu contextual na janela RSS para modificar ou excluir feeds/sites RSS.
• Removida a opção para mover as configurações para a pasta atual; agora o programa faz isso automaticamente (se a pasta do exe se chama "sonarpad portable" ou o exe está em unidade removível, salva na pasta do exe em `config`, senão em `%APPDATA%\\Sonarpad`, com fallback para `config` se a pasta preferida não for gravável).

Versão 0.5.7 - 2026-01-05
Novos recursos
• Adicionada a opção para gravar audiolivros em lote (conversão múltipla de arquivos e pastas).
• Adicionado suporte para arquivos Markdown (.md).
• Adicionada a escolha da codificação ao abrir arquivos de texto.
• Adicionada opção no terminal para anunciar novas linhas com NVDA.
Melhorias
• A gravação de audiolivros agora é salva em MP3 nativo quando selecionado.
• O usuário pode escolher onde inserir o asterisco * que indica modificações não salvas.
• Melhorado o sistema de atualização para ser mais robusto em diferentes cenários.
• Adicionada no menu Editar a função para remover hifens no final da linha (útil para textos OCR).

Versão 0.5.6 - 2026-01-04
Correções
  Melhorado Pesquisar em arquivos: ao pressionar Enter abre o arquivo exatamente no trecho selecionado.
Melhorias
  Suporte a PPT/PPTX.
  Para formatos não textuais, Salvar agora propõe sempre .txt para evitar corromper a formatação (PDF/DOC/DOCX/EPUB/HTML/PPT/PPTX).
  Gravação de podcast do microfone e/ou áudio do sistema (menu Arquivo, Ctrl+Shift+R).

Versão 0.5.5 - 2026-01-03
Novos recursos
• Adicionado um terminal acessível otimizado para muita saída e leitores de tela (Ctrl+Shift+P).
• Adicionada a opção de salvar as configurações na pasta atual (modo portátil).
Correções
• Melhorados os trechos de Pesquisar em arquivos para manter a pré-visualização alinhada com a ocorrência.

Versão 0.5.4 – 2026-01-03
Melhorias
• Correção da função Normalizar espaços em branco (Ctrl+Shift+Enter).
• Suporte a HTML/HTM (abrir como texto).

Versão 0.5.3 – 2026-01-02
Novos recursos
• Adicionado Buscar em arquivos.
• Adicionadas novas ferramentas de texto: Normalizar espaços em branco, Quebra de linha dura e Remover Markdown.
• Adicionadas Estatísticas de texto (Alt+Y).
• Adicionados novos comandos de lista no menu Editar:
• Ordenar itens (Alt+Shift+O)
• Manter itens únicos (Alt+Shift+K)
• Inverter itens (Alt+Shift+Z)
• Adicionados Comentar / Descomentar linhas (Ctrl+Q / Ctrl+Shift+Q).
Localização
• Adicionada a localização em espanhol.
• Adicionada a localização em português.
Melhorias
• Quando um arquivo EPUB está aberto, Salvar muda automaticamente para Salvar como e exporta o conteúdo como .txt para evitar corromper o EPUB.

## 0.5.2 - 2026-01-01
- Adicionado um changelog.
- Adicionadas opções "Abrir com Sonarpad" e associações de arquivos suportados durante a instalação.
- Melhorada a localização de mensagens (erros, diálogos, exportação de audiolivro).
- Adicionada a seleção de partes ao usar "Dividir audiolivro por texto", com a opção "Exigir o marcador no início da linha".
- Adicionada a importação de transcrições do YouTube com seleção de idioma, opção de timestamps e melhorias de foco.

## 0.5.1 - 2025-12-31
- Atualizações automáticas com confirmação, melhorias de erros e notificações.
- Melhorias na exportação de audiolivros (divisão por texto, SAPI5/Media Foundation, controles avançados).
- Melhorias em TTS (pausa e retomada, dicionário de substituições, favoritos).
- Menu Ver e painéis de vozes/favoritos, cor e tamanho de texto.
- Idioma padrão do sistema e melhorias de localização.
- CI e empacotamento Windows (artefatos, MSI/NSIS, cache).

## 0.5.0 - 2025-12-27
- Refatoração modular (editor, manipulação de arquivos, menu, busca).
- Fluxo de compilação e empacotamento para Windows, além de atualizações do README e da licença.
- Correção da navegação TAB na janela de Ajuda.

## 0.5 - 2025-12-27
- Atualização preliminar da versão.

## 0.1.0 - 2025-12-25
- Versão inicial: estrutura do projeto e README.

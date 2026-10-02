export const meta = {
  name: 'shell-panel-review',
  description: 'Revisao geral do shell-panel: lentes por particao e transversais, refutacao de cada achado bloqueante, sintese com lacunas de cobertura',
  whenToUse: 'Revisao geral ou auditoria do shell-panel. args: { config: <conteudo de .claude/review/review.json>, lenses?: ["correcao", ...], partitions?: ["engine", ...] } para rodar so um recorte. Nao corrige nada: devolve achados confirmados para triagem.',
  phases: [
    { title: 'Lentes', detail: 'um agente por (lente, particao) ou por lente transversal' },
    { title: 'Refutar', detail: 'um cetico por achado Critico/Importante, instruido a derruba-lo' },
    { title: 'Sintetizar', detail: 'dedup, cruzamento entre particoes e lacunas de cobertura' },
  ],
}

const CFG = args?.config
if (!CFG?.partitions?.length || !CFG?.lenses?.length) {
  throw new Error('args.config precisa ser o objeto de .claude/review/review.json (com partitions e lenses)')
}
const ROOT = CFG.root
const SKILL = CFG.skillDir
const LENTES_MD = `${SKILL}/references/lentes.md`

const onlyLenses = args?.lenses?.length ? new Set(args.lenses) : null
const onlyParts = args?.partitions?.length ? new Set(args.partitions) : null
const parts = CFG.partitions.filter((p) => !onlyParts || onlyParts.has(p.id))
const lenses = CFG.lenses.filter((l) => !onlyLenses || onlyLenses.has(l.id))

// Uma unidade de trabalho = (lente, particao) ou (lente transversal, repositorio).
const units = []
for (const l of lenses) {
  if (l.scope === 'perPartition') {
    for (const p of parts) units.push({ lens: l, part: p, key: `${l.id}:${p.id}` })
  } else if (!onlyParts) {
    units.push({ lens: l, part: null, key: l.id })
  }
}

const FINDINGS_SCHEMA = {
  type: 'object',
  required: ['achados', 'cobertura'],
  properties: {
    achados: {
      type: 'array',
      maxItems: 15,
      items: {
        type: 'object',
        required: ['titulo', 'severidade', 'arquivo', 'linha', 'trecho', 'cenario', 'evidencia', 'proposta'],
        properties: {
          titulo: { type: 'string' },
          severidade: { type: 'string', enum: ['Critico', 'Importante', 'Menor'] },
          arquivo: { type: 'string', description: 'caminho relativo a raiz do repo, com /' },
          linha: { type: 'integer' },
          trecho: { type: 'string', description: 'literal COPIADO do arquivo (1-3 linhas), nunca reescrito' },
          cenario: { type: 'string', description: 'entrada/estado/sequencia -> resultado errado; "nao construido" se nao conseguiu' },
          evidencia: { type: 'string', description: 'comando rodado + saida colada, no passado; ou "nao medido"' },
          proposta: { type: 'string' },
        },
      },
    },
    cobertura: {
      type: 'object',
      required: ['lido', 'executado', 'naoCoberto'],
      properties: {
        lido: { type: 'array', items: { type: 'string' } },
        executado: { type: 'array', items: { type: 'string' }, description: 'comandos rodados, um por item' },
        naoCoberto: { type: 'string', description: 'o que a pergunta pedia e voce nao verificou, e por que' },
      },
    },
  },
}

const VERDICT_SCHEMA = {
  type: 'object',
  required: ['procede', 'ancoraViva', 'confianca', 'severidade', 'motivo'],
  properties: {
    procede: { type: 'boolean' },
    ancoraViva: { type: 'boolean', description: 'o trecho citado existe no arquivo, na forma citada' },
    confianca: { type: 'string', enum: ['CONFIRMADO', 'PLAUSIVEL'] },
    severidade: { type: 'string', enum: ['Critico', 'Importante', 'Menor'], description: 'a severidade que voce atribui apos checar' },
    motivo: { type: 'string', description: 'o que leu/rodou e o que mostrou, com saida colada' },
  },
}

// Each rule below was broken once by an agent in this repository: an installer run with default
// locations reinstalled the user's real install, and the installer test run under Windows
// PowerShell 5.1 leaked a test entry into the user's PATH.
const SAFETY = `SEGURANCA - a maquina do usuario nao e fixture de teste:
- NUNCA rode install.ps1 ou uninstall.ps1 com os locais padrao (nem como arquivo, nem via irm | iex).
  So via scripts/test-installer.ps1 (lancado a partir do pwsh 7) ou com -InstallDir e
  -TerminalFragmentDir temporarios.
- So UM teste do instalador por vez na maquina: o test-installer.ps1 segura o mutex
  Global\\shell-panel-installer-test; nunca o rode a partir de agentes em paralelo.
- NUNCA escreva no Path de HKCU\\Environment, no historico do PSReadLine, em fragmentos do Windows
  Terminal, em %LOCALAPPDATA%\\Programs\\shell-panel ou em %USERPROFILE%\\.config\\shell-panel*.
- Antes e depois de rodar sessoes PowerShell ou o teste do instalador:
  ${CFG.userState ?? 'user_state.ps1 -Save/-Compare'}
  (nao rode ao mesmo tempo que um teste do instalador em andamento). Se acusar mudanca, PARE e
  relate; nao tente consertar.`

function where(u) {
  if (u.lens.worktree) {
    return `Voce roda numa WORKTREE git propria (seu diretorio atual), na base ${CFG.baseCommit}.
- Execute la: testes de rascunho, mutacao, cargo. Build limpo leva ~90 s; e esperado.
- NUNCA escreva em ${ROOT} (o checkout principal). De la, so LEIA os arquivos da skill.
- NUNCA defina CARGO_TARGET_DIR: os e2e usam target/debug/shell-panel.exe do proprio target.
- Arquivos de rascunho: so tests/zz_review_*.rs. APAGUE todos antes de terminar e confirme com
  \`git status --porcelain\` vazio (worktree limpa e removida sozinha).
- e2e: \`-- --test-threads=1\`; sob carga paralela o primeiro comando de uma sessao ja estourou 15 s.
  Uma falha so conta depois de uma segunda execucao (cole as duas).
- Nao commite.

${SAFETY}`
  }
  return `READ-ONLY no checkout ${ROOT}: nao edite, nao crie arquivo, nao formate, nao commite.
Pode rodar comandos de leitura, git grep, os scripts da skill e \`cargo run -q -- <opcoes>\`.

${SAFETY}`
}

function scope(u) {
  if (!u.part) {
    return `Escopo: o repositorio inteiro (src/, assets/, tests/, README.md, docs/wiki/). Particoes, para
voce citar onde cada achado cai:
${CFG.partitions.map((p) => `  - ${p.id}: ${p.summary}`).join('\n')}`
  }
  return `Particao: ${u.part.id} - ${u.part.summary}
Codigo (exatamente estes):
${u.part.files.map((f) => `  - ${f}`).join('\n')}
Testes desta particao:
${u.part.tests.map((f) => `  - ${f}`).join('\n')}
Leia outros arquivos so para entender chamadas; achado fora da particao vai para outra lente.`
}

function finderPrompt(u) {
  return `Voce e a lente **${u.lens.id}** da revisao geral do shell-panel (Rust, Windows, PowerShell em
ConPTY com dropdown de completion).

${scope(u)}

${where(u)}

ANTES de tudo, leia:
  1. ${LENTES_MD} - a secao "${u.lens.id}": sua UNICA pergunta, onde procurar e as armadilhas
     que ja produziram achado falso aqui.
  2. ${SKILL}/SKILL.md - secoes "Por que este formato", "Regras para todo agente" e "Severidade".

Regras do projeto que um defeito pode violar (com o motivo):
${CFG.projectRules}

Decisoes fechadas - NAO acuse nem proponha reverter:
${CFG.decisionsNotToRelitigate}

Ferramentas (caminho absoluto, funcionam de qualquer diretorio; operam no diretorio atual):
  gate:        ${CFG.gate}
  mutacao:     ${CFG.mutate}
  consumidores:${CFG.consumers}
  (mutate.ps1: 0 = MORTA/coberto, 1 = SOBREVIVEU/descoberto, 2 = inconclusivo, nao e evidencia)

O que decide se seu achado sobrevive (ele sera conferido mecanicamente e por um cetico):
  - arquivo:linha e trecho LITERAL copiado do arquivo. Trecho reescrito = ancora morta = descarte.
  - evidencia no passado com a saida colada. "Se X, entao falharia" nao e evidencia.
  - cenario concreto. Se nao conseguiu construir, escreva "nao construido" e no maximo Menor
    - a menos que a leitura do codigo seja inequivoca, e entao diga qual linha a torna inequivoca.
  - delecao: saida do find_consumers colada. Teste fraco: saida do mutate.ps1 com exit 1 colada.

Zero achados e resultado legitimo e comum. Inventar achado marginal para parecer util e o modo
de falha mais caro aqui. Preencha "cobertura" com honestidade: o que leu, o que rodou e o que
a pergunta pedia e ficou de fora - essa lista decide a proxima rodada.`
}

function refutePrompt(a, u) {
  return `Voce e um cetico da revisao do shell-panel. Sua tarefa e DERRUBAR o achado abaixo.

${where(u)}

Achado da lente "${u.key}":
  severidade: ${a.severidade}
  titulo:     ${a.titulo}
  local:      ${a.arquivo}:${a.linha}
  trecho:     ${a.trecho}
  cenario:    ${a.cenario}
  evidencia:  ${a.evidencia}
  proposta:   ${a.proposta}

Faca, nesta ordem:
1. Abra ${a.arquivo} e confira que o trecho existe LA, na forma citada, perto da linha. Se nao,
   procede=false, ancoraViva=false, e pare.
2. Liste o que o achado ASSUME e cheque cada suposicao na fonte (quem chama, que guarda vem
   antes, que teste ja cobre). Metade dos achados falsos sao verdadeiros sobre um codigo imaginado.
3. Procure o motivo escrito: comentario, commit (\`git log -L\` ou \`git log -S\`), README "Known
   limitations", decisoes fechadas abaixo. Comportamento documentado como limitacao nao e defeito.
4. ${u.lens.worktree
    ? 'REPRODUZA: rode de novo o comando da evidencia (ou escreva o teste de rascunho minimo) e cole a saida. Se a evidencia era uma mutacao, rode o mutate.ps1 voce mesmo.'
    : 'Se o achado depende de comportamento em execucao, diga que nao foi reproduzido e use PLAUSIVEL.'}
   Sem reproducao, confianca=PLAUSIVEL, nunca CONFIRMADO.
5. Reavalie a severidade pelos criterios de ${SKILL}/SKILL.md.

Decisoes fechadas:
${CFG.decisionsNotToRelitigate}

Na duvida, derrube. Um achado falso vira commit ruim; um real perdido reaparece na proxima rodada.`
}

phase('Lentes')
log(`${units.length} unidade(s): ${units.map((u) => u.key).join(', ')}`)

const MAX_REF = CFG.maxRefutations ?? 30
let refutados = 0
let semRefutacao = 0

const porUnidade = await pipeline(
  units,
  (u) =>
    agent(finderPrompt(u), {
      label: `lente:${u.key}`,
      phase: 'Lentes',
      effort: u.lens.effort,
      schema: FINDINGS_SCHEMA,
      ...(u.lens.worktree ? { isolation: 'worktree' } : {}),
    }),
  (r, u) => {
    const achados = (r?.achados ?? []).map((a) => ({ ...a, lente: u.key }))
    const bloqueantes = achados.filter((a) => a.severidade !== 'Menor')
    const menores = achados.filter((a) => a.severidade === 'Menor')
    log(`${u.key}: ${achados.length} achado(s), ${bloqueantes.length} bloqueante(s)`)
    return parallel(
      bloqueantes.map((a, i) => () => {
        if (refutados >= MAX_REF) {
          semRefutacao++
          return Promise.resolve({ ...a, veredito: null })
        }
        refutados++
        return agent(refutePrompt(a, u), {
          label: `refuta:${u.key}:${i + 1}`,
          phase: 'Refutar',
          effort: 'high',
          schema: VERDICT_SCHEMA,
          ...(u.lens.worktree ? { isolation: 'worktree' } : {}),
        }).then((v) => ({ ...a, veredito: v }))
      })
    ).then((julgados) => ({ key: u.key, cobertura: r?.cobertura ?? null, falhou: !r, julgados: julgados.filter(Boolean), menores }))
  }
)

const resultados = porUnidade.filter(Boolean)
const falharam = units.filter((u) => !resultados.find((r) => r.key === u.key && !r.falhou)).map((u) => u.key)
if (falharam.length) log(`ATENCAO: unidade(s) sem resultado: ${falharam.join(', ')}`)
if (semRefutacao) log(`ATENCAO: ${semRefutacao} achado(s) bloqueante(s) acima do limite de ${MAX_REF} NAO foram refutados`)

const julgados = resultados.flatMap((r) => r.julgados)
const ok = (a) => a.veredito && a.veredito.procede && a.veredito.ancoraViva
const confirmados = julgados.filter(ok).map((a) => ({ ...a, severidade: a.veredito.severidade }))
const derrubados = julgados.filter((a) => a.veredito && !ok(a))
const naoRefutados = julgados.filter((a) => !a.veredito)
const menores = resultados.flatMap((r) => r.menores)
log(`${julgados.length} bloqueante(s): ${confirmados.length} confirmados, ${derrubados.length} derrubados, ${naoRefutados.length} sem refutacao; ${menores.length} menor(es)`)

phase('Sintetizar')
const sintese = await agent(
  `Consolide a revisao geral do shell-panel num relatorio markdown em portugues. READ-ONLY.
Pode abrir arquivos em ${ROOT} para desempatar duplicatas; nao reavalie achados.

Achados bloqueantes que sobreviveram a refutacao (JSON):
${JSON.stringify(confirmados, null, 1)}

Achados bloqueantes sem refutacao (limite atingido) - liste em secao propria, marcados NAO VERIFICADOS:
${JSON.stringify(naoRefutados, null, 1)}

Achados Menores (nao refutados; ancora sera conferida mecanicamente depois):
${JSON.stringify(menores, null, 1)}

Derrubados (titulo, local, motivo):
${JSON.stringify(derrubados.map((d) => ({ titulo: d.titulo, local: `${d.arquivo}:${d.linha}`, lente: d.lente, motivo: d.veredito?.motivo })), null, 1)}

Cobertura declarada por unidade:
${JSON.stringify(resultados.map((r) => ({ unidade: r.key, cobertura: r.cobertura })), null, 1)}
Unidades que nao devolveram resultado: ${JSON.stringify(falharam)}

Faca o que nenhuma lente sozinha podia:
1. DEDUPLIQUE: o mesmo defeito achado por lentes diferentes vira UMA entrada com as lentes listadas
   e a maior severidade confirmada.
2. Marque achados que atravessam particoes.
3. LACUNAS: a partir de "naoCoberto" e das unidades que falharam, liste o que a revisao NAO
   verificou. Seja especifico. Isso decide a proxima rodada.
4. NAO decida o que corrigir nem a ordem. A triagem e de quem coordena.

Estrutura: ## Resumo (contagens) / ## Criticos / ## Importantes / ## Menores / ## Nao verificados /
## Derrubados na refutacao / ## Lacunas de cobertura. Cada achado: titulo, arquivo:linha, trecho em
bloco de codigo, cenario, evidencia (resumida, mantendo a linha decisiva da saida), proposta, lentes.
Devolva so o markdown.`,
  { label: 'sintese', phase: 'Sintetizar', effort: 'high' }
)

return {
  unidades: units.map((u) => u.key),
  falharam,
  contagens: {
    bloqueantes: julgados.length,
    confirmados: confirmados.length,
    derrubados: derrubados.length,
    naoRefutados: naoRefutados.length,
    menores: menores.length,
  },
  confirmados,
  naoRefutados,
  menores,
  derrubados: derrubados.map((d) => ({ titulo: d.titulo, arquivo: d.arquivo, linha: d.linha, lente: d.lente, motivo: d.veredito?.motivo })),
  cobertura: resultados.map((r) => ({ unidade: r.key, cobertura: r.cobertura })),
  relatorio: sintese,
}

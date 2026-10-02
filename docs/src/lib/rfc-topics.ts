// The topics the RFC index groups by. Each RFC names its own topic in the
// `topic` frontmatter field, so the grouping travels with the file: a new RFC
// cannot land ungrouped (the content schema requires the field) and cannot be
// filed under a topic that does not exist (the schema takes its enum from here).
//
// Kept in its own module, without Node imports, so the content schema and the
// pure-Node RFC loader can both import it.

export const RFC_TOPICS = [
  {
    id: 'language',
    label: 'Law language and format',
    description:
      'The YAML format, its operations and types, and what a law file may say.',
  },
  {
    id: 'execution',
    label: 'Execution',
    description:
      'How an engine runs laws, calls other laws, and who executes what.',
  },
  {
    id: 'time',
    label: 'Time and versions',
    description:
      'Which version of a law applies on a given date, and how laws end.',
  },
  {
    id: 'corpus',
    label: 'Corpus and pipeline',
    description:
      'Where laws come from, and how the harvester and enricher fill the corpus.',
  },
  {
    id: 'trust',
    label: 'Trust and provenance',
    description:
      'Receipts, traces and notes: how a result can be checked afterwards.',
  },
  {
    id: 'process',
    label: 'Process',
    description: 'How decisions in this project are proposed and recorded.',
  },
] as const;

export type RfcTopicId = (typeof RFC_TOPICS)[number]['id'];

export const RFC_TOPIC_IDS = RFC_TOPICS.map((t) => t.id) as [
  RfcTopicId,
  ...RfcTopicId[],
];

import type { ModelInfo, ModelSource } from "@/bindings";

export type ModelSeriesId = "speakmore" | "handy" | "custom";

export interface ModelSeriesDefinition {
  id: ModelSeriesId;
  source: ModelSource;
  titleKey: string;
  descriptionKey: string;
  badgeKey: string;
}

export interface ModelSeriesGroup {
  definition: ModelSeriesDefinition;
  models: ModelInfo[];
}

const MODEL_SERIES: ModelSeriesDefinition[] = [
  {
    id: "speakmore",
    source: "SpeakMore",
    titleKey: "settings.models.series.speakMore.title",
    descriptionKey: "settings.models.series.speakMore.description",
    badgeKey: "settings.models.series.speakMore.badge",
  },
  {
    id: "handy",
    source: "Handy",
    titleKey: "settings.models.series.handy.title",
    descriptionKey: "settings.models.series.handy.description",
    badgeKey: "settings.models.series.handy.badge",
  },
  {
    id: "custom",
    source: "Custom",
    titleKey: "settings.models.series.custom.title",
    descriptionKey: "settings.models.series.custom.description",
    badgeKey: "settings.models.series.custom.badge",
  },
];

const SERIES_BY_SOURCE = MODEL_SERIES.reduce(
  (acc, definition) => {
    acc[definition.source] = definition;
    return acc;
  },
  {} as Record<ModelSource, ModelSeriesDefinition>,
);

export const getModelSeriesDefinition = (
  model: ModelInfo,
): ModelSeriesDefinition => {
  if (model.is_custom) {
    return SERIES_BY_SOURCE.Custom;
  }
  return SERIES_BY_SOURCE[model.source] ?? SERIES_BY_SOURCE.Handy;
};

export const groupModelsBySeries = (
  models: ModelInfo[],
): ModelSeriesGroup[] => {
  const grouped = new Map<ModelSource, ModelInfo[]>();

  for (const model of models) {
    const definition = getModelSeriesDefinition(model);
    const existing = grouped.get(definition.source) ?? [];
    existing.push(model);
    grouped.set(definition.source, existing);
  }

  return MODEL_SERIES.map((definition) => ({
    definition,
    models: grouped.get(definition.source) ?? [],
  })).filter((group) => group.models.length > 0);
};

export const sortModelsForDisplay = (
  models: ModelInfo[],
  currentModel?: string,
): ModelInfo[] => {
  return [...models].sort((a, b) => {
    if (a.id === currentModel) return -1;
    if (b.id === currentModel) return 1;
    if (a.is_recommended !== b.is_recommended) {
      return a.is_recommended ? -1 : 1;
    }
    if (a.is_downloaded !== b.is_downloaded) {
      return a.is_downloaded ? -1 : 1;
    }
    return 0;
  });
};

// Executable qualification registry. Contract checks and the real-engine suite
// consume this same registry; removing a loader/component breaks qualification.
export const frameworkCases = {
  'react-instantsearch': {
    id: 'react-profile', root: 'InstantSearch',
    components: {searchBox:'SearchBox',hits:'Hits',pagination:'Pagination',hitsPerPage:'HitsPerPage',refinementList:'RefinementList',menu:'Menu',numericMenu:'useNumericMenu',sortBy:'SortBy',stats:'Stats',clearRefinements:'ClearRefinements',currentRefinements:'CurrentRefinements',configure:'Configure',index:'Index',Highlight:'Highlight',Snippet:'Snippet'},
    load: () => import('./react-real-engine.fixture.tsx'),
  },
  'vue-instantsearch': {
    id: 'vue3-profile', root: 'AisInstantSearch',
    components: {searchBox:'AisSearchBox',hits:'AisHits',pagination:'AisPagination',hitsPerPage:'AisHitsPerPage',refinementList:'AisRefinementList',menu:'AisMenu',numericMenu:'AisNumericMenu',sortBy:'AisSortBy',stats:'AisStats',clearRefinements:'AisClearRefinements',currentRefinements:'AisCurrentRefinements',configure:'AisConfigure',index:'AisIndex',Highlight:'AisHighlight',Snippet:'AisSnippet'},
    load: () => import('./vue-real-engine.fixture.ts'),
  },
};
export const frameworkSuite = 'tests/framework-real-engine.test.ts';

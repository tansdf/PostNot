import { browser } from "$app/environment";

import { createStaleGuard } from "$lib/async-stale-guard";
import { searchCollectionEntities } from "$lib/api/commands";
import type { CollectionSearchResult } from "$lib/api/types";

const SEARCH_DEBOUNCE_MS = 120;
const DEFAULT_RESULT_LIMIT = 30;

class CollectionSearchStore {
  query = $state("");
  results = $state.raw<CollectionSearchResult[]>([]);
  isSearching = $state(false);
  activeIndex = $state(-1);
  errorText = $state("");
  searchTimer: ReturnType<typeof setTimeout> | null = null;
  staleGuard = createStaleGuard();

  get isActive() {
    return this.query.trim().length > 0;
  }

  get activeResult(): CollectionSearchResult | null {
    if (this.activeIndex < 0 || this.activeIndex >= this.results.length) {
      return null;
    }

    return this.results[this.activeIndex] ?? null;
  }

  setQuery(value: string) {
    this.query = value;
    this.scheduleSearch();
  }

  clear() {
    this.query = "";
    this.errorText = "";
    this.results = [];
    this.activeIndex = -1;
    this.isSearching = false;
    this.bumpStaleGuard();

    if (this.searchTimer) {
      clearTimeout(this.searchTimer);
      this.searchTimer = null;
    }
  }

  moveSelection(direction: -1 | 1) {
    if (this.results.length === 0) {
      this.activeIndex = -1;
      return;
    }

    const nextIndex =
      this.activeIndex < 0
        ? 0
        : (this.activeIndex + direction + this.results.length) % this.results.length;

    this.activeIndex = nextIndex;
  }

  setActiveIndex(index: number) {
    if (index < 0 || index >= this.results.length) {
      return;
    }

    this.activeIndex = index;
  }

  async refresh() {
    if (!this.isActive) {
      return;
    }

    await this.runSearch(this.query.trim());
  }

  private scheduleSearch() {
    const trimmedQuery = this.query.trim();

    if (this.searchTimer) {
      clearTimeout(this.searchTimer);
      this.searchTimer = null;
    }

    if (!trimmedQuery) {
      this.results = [];
      this.activeIndex = -1;
      this.errorText = "";
      this.isSearching = false;
      this.bumpStaleGuard();
      return;
    }

    this.isSearching = true;
    this.errorText = "";

    this.searchTimer = setTimeout(() => {
      this.searchTimer = null;
      void this.runSearch(trimmedQuery);
    }, SEARCH_DEBOUNCE_MS);
  }

  private bumpStaleGuard() {
    this.staleGuard.next();
  }

  private async runSearch(query: string) {
    if (!browser) {
      this.results = [];
      this.activeIndex = -1;
      this.isSearching = false;
      return;
    }

    const seq = this.staleGuard.next();

    try {
      const results = await searchCollectionEntities(query, DEFAULT_RESULT_LIMIT);
      if (this.staleGuard.isStale(seq)) {
        return;
      }

      this.results = results;
      this.activeIndex = results.length > 0 ? 0 : -1;
      this.errorText = "";
    } catch (error) {
      if (this.staleGuard.isStale(seq)) {
        return;
      }

      this.results = [];
      this.activeIndex = -1;
      this.errorText = error instanceof Error ? error.message : String(error);
    } finally {
      if (!this.staleGuard.isStale(seq)) {
        this.isSearching = false;
      }
    }
  }
}

export const collectionSearch = new CollectionSearchStore();

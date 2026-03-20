import { browser } from "$app/environment";
import { derived, get, writable } from "svelte/store";

import {
  createCollection,
  deleteCollection,
  deleteSavedRequest,
  listCollections,
  listSavedRequests,
  saveRequestToCollection,
  updateCollection as updateCollectionCommand,
  updateSavedRequest as updateSavedRequestCommand
} from "$lib/api/commands";
import type { CollectionSummary, CreateCollectionInput, RequestDraft, SavedRequestSummary } from "$lib/api/types";

type CollectionsState = {
  initialized: boolean;
  isCollectionsLoading: boolean;
  isSavedRequestsLoading: boolean;
  isCreatingCollection: boolean;
  isSavingRequest: boolean;
  selectedCollectionId: string;
  collections: CollectionSummary[];
  savedRequestsByCollection: Record<string, SavedRequestSummary[]>;
  pendingDeleteCollectionId: string;
  pendingDeleteSavedRequestId: string;
  errorText: string;
};

const initialState: CollectionsState = {
  initialized: false,
  isCollectionsLoading: false,
  isSavedRequestsLoading: false,
  isCreatingCollection: false,
  isSavingRequest: false,
  selectedCollectionId: "",
  collections: [],
  savedRequestsByCollection: {},
  pendingDeleteCollectionId: "",
  pendingDeleteSavedRequestId: "",
  errorText: ""
};

const store = writable<CollectionsState>(initialState);

function patchState(patch: Partial<CollectionsState>) {
  store.update((state) => ({ ...state, ...patch }));
}

export const collectionsState = {
  subscribe: store.subscribe
};

export const selectedCollection = derived(collectionsState, ($state) =>
  $state.collections.find((collection) => collection.id === $state.selectedCollectionId) ?? null
);

export const selectedSavedRequests = derived(collectionsState, ($state) =>
  $state.selectedCollectionId ? $state.savedRequestsByCollection[$state.selectedCollectionId] ?? [] : []
);

export function resetCollectionsError() {
  patchState({ errorText: "" });
}

export async function ensureCollectionsLoaded(preferredCollectionId = get(store).selectedCollectionId) {
  if (!browser) {
    return;
  }

  const state = get(store);
  if (state.initialized && !preferredCollectionId) {
    return;
  }

  await loadCollections(preferredCollectionId);
}

export async function loadCollections(preferredCollectionId = get(store).selectedCollectionId) {
  if (!browser) {
    return;
  }

  patchState({ isCollectionsLoading: true });

  try {
    const collections = await listCollections();
    const nextSelectedCollectionId =
      preferredCollectionId && collections.some((collection) => collection.id === preferredCollectionId)
        ? preferredCollectionId
        : collections[0]?.id ?? "";

    patchState({
      collections,
      selectedCollectionId: nextSelectedCollectionId,
      initialized: true,
      errorText: ""
    });

    if (nextSelectedCollectionId) {
      await loadSavedRequests(nextSelectedCollectionId);
    } else {
      patchState({ savedRequestsByCollection: {} });
    }
  } catch (error) {
    patchState({
      initialized: true,
      errorText: error instanceof Error ? error.message : String(error)
    });
  } finally {
    patchState({ isCollectionsLoading: false });
  }
}

export async function loadSavedRequests(collectionId: string) {
  if (!browser || !collectionId) {
    return;
  }

  patchState({ isSavedRequestsLoading: true });

  try {
    const savedRequests = await listSavedRequests(collectionId);
    store.update((state) => ({
      ...state,
      savedRequestsByCollection: {
        ...state.savedRequestsByCollection,
        [collectionId]: savedRequests
      },
      errorText: ""
    }));
  } catch (error) {
    patchState({ errorText: error instanceof Error ? error.message : String(error) });
  } finally {
    patchState({ isSavedRequestsLoading: false });
  }
}

export async function selectCollection(collectionId: string) {
  patchState({ selectedCollectionId: collectionId });
  await loadSavedRequests(collectionId);
}

export async function createBlankCollection() {
  patchState({ isCreatingCollection: true });

  try {
    const collection = await createCollection({
      name: "Untitled collection",
      description: ""
    });

    await loadCollections(collection.id);
    return collection;
  } catch (error) {
    patchState({ errorText: error instanceof Error ? error.message : String(error) });
    return null;
  } finally {
    patchState({ isCreatingCollection: false });
  }
}

export async function saveCollectionDetails(collectionId: string, input: CreateCollectionInput) {
  try {
    const collection = await updateCollectionCommand(collectionId, input);
    await loadCollections(collection.id);
    return collection;
  } catch (error) {
    patchState({ errorText: error instanceof Error ? error.message : String(error) });
    return null;
  }
}

export async function removeCollection(collectionId: string) {
  patchState({ pendingDeleteCollectionId: collectionId });

  try {
    await deleteCollection(collectionId);

    store.update((state) => {
      const savedRequestsByCollection = { ...state.savedRequestsByCollection };
      delete savedRequestsByCollection[collectionId];

      return {
        ...state,
        savedRequestsByCollection
      };
    });

    const preferredCollectionId = get(store).selectedCollectionId === collectionId ? "" : get(store).selectedCollectionId;
    await loadCollections(preferredCollectionId);
  } catch (error) {
    patchState({ errorText: error instanceof Error ? error.message : String(error) });
  } finally {
    patchState({ pendingDeleteCollectionId: "" });
  }
}

export async function saveNewRequest(collectionId: string, request: RequestDraft) {
  patchState({ isSavingRequest: true });

  try {
    const savedRequest = await saveRequestToCollection(collectionId, request);
    await Promise.all([loadCollections(collectionId), loadSavedRequests(collectionId)]);
    return savedRequest;
  } catch (error) {
    patchState({ errorText: error instanceof Error ? error.message : String(error) });
    return null;
  } finally {
    patchState({ isSavingRequest: false });
  }
}

export async function updateExistingSavedRequest(itemId: string, collectionId: string, request: RequestDraft) {
  patchState({ isSavingRequest: true });

  try {
    const savedRequest = await updateSavedRequestCommand(itemId, request);
    await Promise.all([loadCollections(collectionId), loadSavedRequests(collectionId)]);
    return savedRequest;
  } catch (error) {
    patchState({ errorText: error instanceof Error ? error.message : String(error) });
    return null;
  } finally {
    patchState({ isSavingRequest: false });
  }
}

export async function removeSavedRequestItem(collectionId: string, itemId: string) {
  patchState({ pendingDeleteSavedRequestId: itemId });

  try {
    await deleteSavedRequest(itemId);
    await Promise.all([loadCollections(collectionId), loadSavedRequests(collectionId)]);
  } catch (error) {
    patchState({ errorText: error instanceof Error ? error.message : String(error) });
  } finally {
    patchState({ pendingDeleteSavedRequestId: "" });
  }
}

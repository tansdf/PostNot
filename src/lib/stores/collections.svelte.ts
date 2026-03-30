import { browser } from "$app/environment";
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
import { notifications } from "$lib/stores/notifications.svelte";

class CollectionsStore {
  initialized = $state(false);
  isCollectionsLoading = $state(false);
  isSavedRequestsLoading = $state(false);
  isCreatingCollection = $state(false);
  isSavingRequest = $state(false);
  selectedCollectionId = $state("");
  collections = $state.raw<CollectionSummary[]>([]);
  savedRequestsByCollection = $state.raw<Record<string, SavedRequestSummary[]>>({});
  pendingDeleteCollectionId = $state("");
  pendingDeleteSavedRequestId = $state("");
  errorText = $state("");

  get selectedCollection(): CollectionSummary | null {
    return this.collections.find((c) => c.id === this.selectedCollectionId) ?? null;
  }

  get selectedSavedRequests(): SavedRequestSummary[] {
    return this.selectedCollectionId
      ? this.savedRequestsByCollection[this.selectedCollectionId] ?? []
      : [];
  }

  resetError() {
    this.errorText = "";
  }

  async ensureLoaded(preferredCollectionId = this.selectedCollectionId) {
    if (!browser) return;
    if (this.initialized && !preferredCollectionId) return;
    await this.loadCollections(preferredCollectionId);
  }

  async loadCollections(preferredCollectionId = this.selectedCollectionId) {
    if (!browser) return;
    this.isCollectionsLoading = true;

    try {
      const fetched = await listCollections();
      const nextId =
        preferredCollectionId && fetched.some((c) => c.id === preferredCollectionId)
          ? preferredCollectionId
          : fetched[0]?.id ?? "";

      this.collections = fetched;
      this.selectedCollectionId = nextId;
      this.initialized = true;
      this.errorText = "";

      if (nextId) {
        await this.loadSavedRequests(nextId);
      } else {
        this.savedRequestsByCollection = {};
      }
    } catch (error) {
      this.initialized = true;
      this.errorText = error instanceof Error ? error.message : String(error);
    } finally {
      this.isCollectionsLoading = false;
    }
  }

  async loadSavedRequests(collectionId: string) {
    if (!browser || !collectionId) return;
    this.isSavedRequestsLoading = true;

    try {
      const savedRequests = await listSavedRequests(collectionId);
      this.savedRequestsByCollection = {
        ...this.savedRequestsByCollection,
        [collectionId]: savedRequests
      };
      this.errorText = "";
    } catch (error) {
      this.errorText = error instanceof Error ? error.message : String(error);
    } finally {
      this.isSavedRequestsLoading = false;
    }
  }

  async selectCollection(collectionId: string) {
    this.selectedCollectionId = collectionId;
    await this.loadSavedRequests(collectionId);
  }

  async createBlankCollection() {
    this.isCreatingCollection = true;

    try {
      const collection = await createCollection({
        name: "Untitled collection",
        description: ""
      });
      await this.loadCollections(collection.id);
      notifications.success(collection.name, "Collection created");
      return collection;
    } catch (error) {
      this.errorText = error instanceof Error ? error.message : String(error);
      return null;
    } finally {
      this.isCreatingCollection = false;
    }
  }

  async saveDetails(collectionId: string, input: CreateCollectionInput) {
    try {
      const collection = await updateCollectionCommand(collectionId, input);
      await this.loadCollections(collection.id);
      notifications.success(collection.name, "Collection saved");
      return collection;
    } catch (error) {
      this.errorText = error instanceof Error ? error.message : String(error);
      return null;
    }
  }

  async removeCollection(collectionId: string) {
    this.pendingDeleteCollectionId = collectionId;
    const collectionName = this.collections.find((item) => item.id === collectionId)?.name ?? "Collection";

    try {
      await deleteCollection(collectionId);
      const { [collectionId]: _, ...rest } = this.savedRequestsByCollection;
      this.savedRequestsByCollection = rest;

      const preferredId = this.selectedCollectionId === collectionId ? "" : this.selectedCollectionId;
      await this.loadCollections(preferredId);
      notifications.success(collectionName, "Collection deleted");
    } catch (error) {
      this.errorText = error instanceof Error ? error.message : String(error);
    } finally {
      this.pendingDeleteCollectionId = "";
    }
  }

  async saveNewRequest(collectionId: string, request: RequestDraft) {
    this.isSavingRequest = true;

    try {
      const savedRequest = await saveRequestToCollection(collectionId, request);
      await Promise.all([this.loadCollections(collectionId), this.loadSavedRequests(collectionId)]);
      notifications.success(savedRequest.name, "Request saved");
      return savedRequest;
    } catch (error) {
      this.errorText = error instanceof Error ? error.message : String(error);
      return null;
    } finally {
      this.isSavingRequest = false;
    }
  }

  async updateExistingSavedRequest(itemId: string, collectionId: string, request: RequestDraft) {
    this.isSavingRequest = true;

    try {
      const savedRequest = await updateSavedRequestCommand(itemId, request);
      await Promise.all([this.loadCollections(collectionId), this.loadSavedRequests(collectionId)]);
      notifications.success(savedRequest.name, "Request updated");
      return savedRequest;
    } catch (error) {
      this.errorText = error instanceof Error ? error.message : String(error);
      return null;
    } finally {
      this.isSavingRequest = false;
    }
  }

  async removeSavedRequestItem(collectionId: string, itemId: string) {
    this.pendingDeleteSavedRequestId = itemId;
    const savedRequestName =
      (this.savedRequestsByCollection[collectionId] ?? []).find((item) => item.id === itemId)?.name || "Saved request";

    try {
      await deleteSavedRequest(itemId);
      await Promise.all([this.loadCollections(collectionId), this.loadSavedRequests(collectionId)]);
      notifications.success(savedRequestName, "Saved request deleted");
    } catch (error) {
      this.errorText = error instanceof Error ? error.message : String(error);
    } finally {
      this.pendingDeleteSavedRequestId = "";
    }
  }
}

export const collections = new CollectionsStore();

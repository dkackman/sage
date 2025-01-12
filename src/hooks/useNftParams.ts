import { useSearchParams } from 'react-router-dom';
import { useLocalStorage } from 'usehooks-ts';

const NFT_VIEW_STORAGE_KEY = 'sage-wallet-nft-view';
const NFT_HIDDEN_STORAGE_KEY = 'sage-wallet-nft-hidden';

export enum NftView {
  Name = 'name',
  Recent = 'recent',
  Collection = 'collection',
  Did = 'did',
}

export enum NftSortMode {
  Name = 'name',
  Recent = 'recent',
}

export enum NftGroupMode {
  None = 'none',
  Collection = 'collection',
  Did = 'did',
}

export interface NftParams {
  pageSize: number;
  page: number;
  sort: NftSortMode;
  group: NftGroupMode;
  showHidden: boolean;
  query: string | null;
}

export type SetNftParams = (params: Partial<NftParams>) => void;

function parseView(view: string | null): NftView {
  switch (view) {
    case 'name':
      return NftView.Name;
    case 'recent':
      return NftView.Recent;
    case 'collection':
      return NftView.Collection;
    case 'did':
      return NftView.Did;
    default:
      return NftView.Name;
  }
}

export function useNftParams(): [NftParams, SetNftParams] {
  const [params, setParams] = useSearchParams();
  const [storedSort, setStoredSort] = useLocalStorage<NftSortMode>(
    NFT_VIEW_STORAGE_KEY,
    NftSortMode.Name,
  );
  const [storedShowHidden, setStoredShowHidden] = useLocalStorage<boolean>(
    NFT_HIDDEN_STORAGE_KEY,
    false,
  );

  const pageSize = parseInt(params.get('pageSize') ?? '12');
  const page = parseInt(params.get('page') ?? '1');
  const sort = (params.get('sort') as NftSortMode) ?? storedSort;
  const group = (params.get('group') as NftGroupMode) ?? NftGroupMode.None;
  const showHidden =
    (params.get('showHidden') ?? storedShowHidden.toString()) === 'true';
  const query = params.get('query');

  const updateParams = ({
    page,
    sort,
    group,
    showHidden,
    query,
  }: Partial<NftParams>) => {
    setParams(
      (prev) => {
        const next = new URLSearchParams(prev);

        if (page !== undefined) {
          next.set('page', page.toString());
        }

        if (sort !== undefined) {
          next.set('sort', sort);
          setStoredSort(sort);
        }

        if (group !== undefined) {
          next.set('group', group);
        }

        if (showHidden !== undefined) {
          next.set('showHidden', showHidden.toString());
          setStoredShowHidden(showHidden);
        }

        if (query !== undefined) {
          if (query) {
            next.set('query', query);
          } else {
            next.delete('query');
          }
        }

        return next;
      },
      { replace: true },
    );
  };

  return [{ pageSize, page, sort, group, showHidden, query }, updateParams];
}

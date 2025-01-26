import {
  NftGroupMode,
  NftParams,
  NftSortMode,
  SetNftParams,
} from '@/hooks/useNftParams';
import { t } from '@lingui/core/macro';
import { Trans } from '@lingui/react/macro';
import {
  ArrowDownAz,
  ArrowLeftIcon,
  Clock2,
  CopyPlus,
  EyeIcon,
  EyeOff,
  LayoutGrid,
  LibraryBigIcon,
  Paintbrush,
  SearchIcon,
  UserIcon,
  XIcon,
} from 'lucide-react';
import { useNavigate, useParams } from 'react-router-dom';
import { Button } from './ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from './ui/dropdown-menu';
import { Input } from './ui/input';
import { Pagination } from './Pagination';

export interface NftOptionsProps {
  isCollection?: boolean;
  params: NftParams;
  setParams: SetNftParams;
  multiSelect: boolean;
  setMultiSelect: (value: boolean) => void;
  className?: string;
  isLoading?: boolean;
  canLoadMore: boolean;
  total: number;
}

export function NftOptions({
  isCollection,
  params: { page, sort, group, showHidden, query, pageSize },
  setParams,
  multiSelect,
  setMultiSelect,
  className,
  total,
}: NftOptionsProps) {
  const { collection_id, owner_did, minter_did } = useParams();
  const navigate = useNavigate();
  const isFilteredView = Boolean(collection_id || owner_did || minter_did);
  const allowSearch = group === NftGroupMode.None || isFilteredView;
  const handleBack = () => {
    if (collection_id) {
      setParams({ group: NftGroupMode.Collection, page: 1 });
    } else if (owner_did) {
      setParams({ group: NftGroupMode.OwnerDid, page: 1 });
    } else if (minter_did) {
      setParams({ group: NftGroupMode.MinterDid, page: 1 });
    }
    navigate('/nfts');
  };

  // For sort button
  const sortMode = sort === NftSortMode.Name ? 'name' : 'recent';
  const sortLabel = t`Sort options: currently sorted by ${sortMode}`;

  // For group button
  const groupMode =
    group === NftGroupMode.Collection
      ? 'collections'
      : group === NftGroupMode.OwnerDid
        ? 'owners'
        : group === NftGroupMode.MinterDid
          ? 'minters'
          : 'no grouping';
  const groupLabel = t`Group options: currently grouped by ${groupMode}`;

  return (
    <div
      className={`flex flex-col gap-4 ${className}`}
      role='toolbar'
      aria-label={t`NFT filtering and sorting options`}
    >
      <div className='relative flex-1' role='search'>
        <div className='relative'>
          <SearchIcon
            className='absolute left-2 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground'
            aria-hidden='true'
          />
          <Input
            value={query ?? ''}
            aria-label={t`Search NFTs by name...`}
            title={t`Search NFTs by name...`}
            placeholder={t`Search NFTs by name...`}
            onChange={(e) => setParams({ query: e.target.value, page: 1 })}
            className='w-full pl-8 pr-8'
            disabled={!allowSearch}
            aria-disabled={!allowSearch}
          />
        </div>
        {query && (
          <Button
            variant='ghost'
            size='icon'
            title={t`Clear search`}
            aria-label={t`Clear search`}
            className='absolute right-0 top-0 h-full px-2 hover:bg-transparent'
            onClick={() => setParams({ query: '', page: 1 })}
            disabled={!allowSearch}
          >
            <XIcon className='h-4 w-4' aria-hidden='true' />
          </Button>
        )}
      </div>

      <div className='flex items-center justify-between'>
        <Pagination
          page={page}
          total={total}
          pageSize={pageSize}
          onPageChange={(newPage) => setParams({ page: newPage })}
          onPageSizeChange={(newSize) =>
            setParams({ pageSize: newSize, page: 1 })
          }
          pageSizeOptions={[24, 48, 72, 96]}
          compact={true}
        />

        <div
          className='flex gap-2 items-center'
          role='toolbar'
          aria-label={t`NFT view options`}
        >
          {isFilteredView && (
            <Button
              variant='outline'
              size='icon'
              onClick={handleBack}
              aria-label={t`Back to groups`}
              title={t`Back to groups`}
            >
              <ArrowLeftIcon className='h-4 w-4' aria-hidden='true' />
            </Button>
          )}

          <Button
            variant='outline'
            size='icon'
            onClick={() => setMultiSelect(!multiSelect)}
            aria-label={t`Toggle multi-select`}
            title={t`Toggle multi-select`}
            disabled={!(group === NftGroupMode.None || isFilteredView)}
          >
            <CopyPlus
              className={`h-4 w-4 ${multiSelect ? 'text-green-600 dark:text-green-400' : ''}`}
              aria-hidden='true'
            />
          </Button>

          <Button
            variant='outline'
            size='icon'
            onClick={() => setParams({ showHidden: !showHidden })}
            aria-label={
              showHidden ? t`Hide hidden items` : t`Show hidden items`
            }
            title={showHidden ? t`Hide hidden items` : t`Show hidden items`}
            disabled={
              !(
                group === NftGroupMode.None ||
                isFilteredView ||
                group === NftGroupMode.Collection
              )
            }
          >
            {showHidden ? (
              <EyeIcon className='h-4 w-4' aria-hidden='true' />
            ) : (
              <EyeOff className='h-4 w-4' aria-hidden='true' />
            )}
          </Button>

          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                variant='outline'
                size='icon'
                aria-label={sortLabel}
                title={sortLabel}
                disabled={!(group === NftGroupMode.None || isFilteredView)}
              >
                {sort === NftSortMode.Name ? (
                  <ArrowDownAz className='h-4 w-4' aria-hidden='true' />
                ) : (
                  <Clock2 className='h-4 w-4' aria-hidden='true' />
                )}
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align='end'>
              <DropdownMenuGroup>
                <DropdownMenuItem
                  className='cursor-pointer'
                  onClick={() => setParams({ sort: NftSortMode.Name, page: 1 })}
                  aria-label={t`Sort by name`}
                >
                  <ArrowDownAz className='mr-2 h-4 w-4' aria-hidden='true' />
                  <span>
                    <Trans>Name</Trans>
                  </span>
                </DropdownMenuItem>
                <DropdownMenuItem
                  className='cursor-pointer'
                  onClick={() =>
                    setParams({ sort: NftSortMode.Recent, page: 1 })
                  }
                  aria-label={t`Sort by recent`}
                >
                  <Clock2 className='mr-2 h-4 w-4' aria-hidden='true' />
                  <span>
                    <Trans>Recent</Trans>
                  </span>
                </DropdownMenuItem>
              </DropdownMenuGroup>
            </DropdownMenuContent>
          </DropdownMenu>

          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                variant='outline'
                size='icon'
                aria-label={groupLabel}
                title={groupLabel}
                disabled={isCollection || isFilteredView}
              >
                {group === NftGroupMode.Collection ? (
                  <LibraryBigIcon className='h-4 w-4' aria-hidden='true' />
                ) : group === NftGroupMode.OwnerDid ? (
                  <UserIcon className='h-4 w-4' aria-hidden='true' />
                ) : group === NftGroupMode.MinterDid ? (
                  <Paintbrush className='h-4 w-4' aria-hidden='true' />
                ) : (
                  <LayoutGrid className='h-4 w-4' aria-hidden='true' />
                )}
              </Button>
            </DropdownMenuTrigger>

            <DropdownMenuContent align='end'>
              <DropdownMenuGroup>
                <DropdownMenuItem
                  className='cursor-pointer'
                  onClick={(e) => {
                    e.stopPropagation();
                    setParams({
                      page: 1,
                      group: NftGroupMode.None,
                    });
                  }}
                  aria-label={t`No Grouping`}
                >
                  <LayoutGrid className='mr-2 h-4 w-4' aria-hidden='true' />
                  <span>
                    <Trans>No Grouping</Trans>
                  </span>
                </DropdownMenuItem>

                <DropdownMenuItem
                  className='cursor-pointer'
                  onClick={(e) => {
                    e.stopPropagation();
                    setParams({
                      page: 1,
                      group: NftGroupMode.Collection,
                      query: '',
                    });
                  }}
                  aria-label={t`Group by Collections`}
                >
                  <LibraryBigIcon className='mr-2 h-4 w-4' aria-hidden='true' />
                  <span>
                    <Trans>Group by Collections</Trans>
                  </span>
                </DropdownMenuItem>

                <DropdownMenuItem
                  className='cursor-pointer'
                  onClick={(e) => {
                    e.stopPropagation();
                    setParams({
                      page: 1,
                      group: NftGroupMode.OwnerDid,
                      query: '',
                    });
                  }}
                  aria-label={t`Group by Owners`}
                >
                  <UserIcon className='mr-2 h-4 w-4' aria-hidden='true' />
                  <span>
                    <Trans>Group by Owners</Trans>
                  </span>
                </DropdownMenuItem>

                <DropdownMenuItem
                  className='cursor-pointer'
                  onClick={(e) => {
                    e.stopPropagation();
                    setParams({
                      page: 1,
                      group: NftGroupMode.MinterDid,
                      query: '',
                    });
                  }}
                  aria-label={t`Group by Minters`}
                >
                  <Paintbrush className='mr-2 h-4 w-4' aria-hidden='true' />
                  <span>
                    <Trans>Group by Minters</Trans>
                  </span>
                </DropdownMenuItem>
              </DropdownMenuGroup>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </div>
    </div>
  );
}

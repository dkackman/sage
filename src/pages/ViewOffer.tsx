import { commands, OfferSummary, TakeOfferResponse } from '@/bindings';
import ConfirmationDialog from '@/components/ConfirmationDialog';
import { TakeOfferConfirmation } from '@/components/confirmations/TakeOfferConfirmation';
import Container from '@/components/Container';
import Header from '@/components/Header';
import { Loading } from '@/components/Loading';
import { OfferCard } from '@/components/OfferCard';
import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import { FeeAmountInput } from '@/components/ui/masked-input';
import { CustomError } from '@/contexts/ErrorContext';
import { useErrors } from '@/hooks/useErrors';
import { resolveOfferData } from '@/lib/offerData';
import { toMojos } from '@/lib/utils';
import { useWalletState } from '@/state';
import { t } from '@lingui/core/macro';
import { Trans } from '@lingui/react/macro';
import { useCallback, useEffect, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';

export function ViewOffer() {
  const { offer } = useParams();
  const { addError } = useErrors();
  const walletState = useWalletState();
  const navigate = useNavigate();

  const [isLoading, setIsLoading] = useState(true);
  const [loadingStatus, setLoadingStatus] = useState(t`Initializing...`);
  const [summary, setSummary] = useState<OfferSummary | null>(null);
  const [response, setResponse] = useState<TakeOfferResponse | null>(null);
  const [fee, setFee] = useState('');
  const [resolvedOffer, setResolvedOffer] = useState<string | null>(null);

  const resolveOffer = useCallback(async () => {
    if (!offer) return;

    setIsLoading(true);
    setLoadingStatus(t`Fetching offer details...`);

    try {
      const resolvedOffer = await resolveOfferData(offer);
      setResolvedOffer(resolvedOffer);

      const data = await commands.viewOffer({ offer: resolvedOffer });
      setSummary(data.offer);
      setLoadingStatus(t`Processing offer data...`);
    } catch (error) {
      addError(error as CustomError);
      navigate('/offers');
    } finally {
      setIsLoading(false);
    }
  }, [offer, addError, navigate]);

  useEffect(() => {
    resolveOffer();
  }, [resolveOffer]);

  const importOffer = async () => {
    if (!resolvedOffer) return;

    try {
      await commands.importOffer({ offer: resolvedOffer });
      navigate('/offers');
    } catch (error) {
      addError(error as CustomError);
    }
  };

  const take = async () => {
    if (!resolvedOffer) return;

    try {
      const result = await commands.takeOffer({
        offer: resolvedOffer,
        fee: toMojos(fee || '0', walletState.sync.unit.precision),
      });
      setResponse(result);
    } catch (error) {
      addError(error as CustomError);
    }
  };

  return (
    <>
      <Header title='View Offer' />

      <Container>
        {isLoading ? (
          <Loading className='my-8' text={loadingStatus} />
        ) : (
          summary && (
            <>
              <OfferCard
                summary={summary}
                content={
                  <div className='flex flex-col space-y-1.5'>
                    <Label htmlFor='fee'>
                      <Trans>Network Fee</Trans>
                    </Label>
                    <FeeAmountInput
                      id='fee'
                      className='pr-12'
                      onValueChange={(values) => setFee(values.value)}
                      onKeyDown={(event) => {
                        if (event.key === 'Enter') {
                          event.preventDefault();
                          take();
                        }
                      }}
                    />
                  </div>
                }
              />

              <div className='mt-4 flex gap-2'>
                <Button variant='outline' onClick={importOffer}>
                  <Trans>Import Offer</Trans>
                </Button>

                <Button onClick={take}>
                  <Trans>Take Offer</Trans>
                </Button>
              </div>
            </>
          )
        )}
      </Container>

      <ConfirmationDialog
        showRecipientDetails={false}
        response={response}
        close={() => setResponse(null)}
        onConfirm={() => navigate('/')}
        additionalData={{
          title: t`Take Offer`,
          content: response && summary && (
            <TakeOfferConfirmation offer={summary} />
          ),
        }}
      />
    </>
  );
}

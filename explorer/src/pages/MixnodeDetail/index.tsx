import * as React from 'react';
import { Box, Grid, Typography } from '@mui/material';
import { MixnodesTable } from '../../components/MixnodesTable';
import { MainContext } from 'src/context/main';
import { useParams } from 'react-router-dom';
import { ContentCard } from 'src/components/ContentCard';

export const PageMixnodeDetail: React.FC = () => {
    const { fetchMixnodeById, mixnodeDetailInfo } = React.useContext(MainContext);
    let { id }: any = useParams();


    React.useEffect(() => {
        if(id) {
            fetchMixnodeById(id)
        }
        console.log("mixnodeDetailInfo is back ", mixnodeDetailInfo);
    }, [id, fetchMixnodeById]) 
    return (
        <>
            <Box component='main' sx={{ flexGrow: 1 }}>
                <Grid container spacing={2}>
                    <Grid item xs={12}>
                        <Typography>
                            Mixnode Detail
                        </Typography>
                    </Grid>
                    <Grid item xs={12}>
                        {mixnodeDetailInfo && (
                            <MixnodesTable
                                mixnodes={{
                                    data: [ mixnodeDetailInfo ],
                                }}
                            />
                        )}
                    </Grid>

                    <Grid item xs={12}>
                        <ContentCard title='Bond Breakdown'>
                            <p>i am the bond breakdown section with lots of stuff</p>
                        </ContentCard>
                    </Grid>
                    <Grid item xs={12} md={6}>
                        <ContentCard title='Mixnode Stats'>
                            <p>I am the mixnode stats</p>
                        </ContentCard>
                    </Grid>
                    <Grid item xs={12} md={6}>
                        <ContentCard title='uptine story'>
                            <p>I am the uptime story</p>
                        </ContentCard>
                    </Grid>
                </Grid>
            </Box>
        </>
    )
}

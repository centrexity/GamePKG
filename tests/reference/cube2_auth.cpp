#include "cube.h"
int main()
{
    vector<char> priv,pub,challenge,answer;
    genprivkey("GamePKG public regression credential, never a real account",priv,pub);
    void *key=parsepubkey(pub.getbuf());
    const unsigned char seed[]={1,2,3,4,5,6,7,8};
    void *expected=genchallenge(key,seed,sizeof(seed),challenge);
    if(!answerchallenge(priv.getbuf(),challenge.getbuf(),answer) || !checkchallenge(answer.getbuf(),expected)) return 1;
    printf("%s\n%s\n%s\n",priv.getbuf(),challenge.getbuf(),answer.getbuf());
    freechallenge(expected);freepubkey(key);
}
